//! LM engine: Jieba-segmented candidate generation with KenLM perplexity
//! scoring (spec 056). When no KenLM model is loaded, falls back to a
//! heuristic confidence gate (spec 055).
//!
//! ## KenLM scoring path (spec 056)
//!
//! 1. **Model resolution** (`ensure_model`) — configured path; return
//!    `Err(ModelMissing/ModelLoadFailed)` so `--engine lm` exits 1 (FR-L6).
//! 2. **Scoring** (`perplexity`) — calls `super::kenlm_ffi::KenLmModel`.
//! 3. **Gates** — relative PPL improvement ≥ threshold AND no new OOV (FR-L4).
//! 4. When `self.model` is populated, `propose()` uses KenLM scoring. Otherwise
//!    it falls back to the heuristic confidence gate.

use std::collections::HashMap;
use std::path::Path;

use jieba_rs::Jieba;

use crate::error::{MfError, Result};
use crate::model::term::{EngineKind, FixKind, Term};

use super::kenlm_ffi::KenLmModel;
use super::{CorrectCtx, CorrectionProposal, Corrector};

/// Default minimum relative PPL improvement required to auto-fix (FR-L4).
pub const DEFAULT_PPL_THRESHOLD: f64 = 0.20;
/// Max Jieba candidates retained per tone-stripped-pinyin key (FR-L3).
pub const MAX_CANDIDATES_PER_KEY: usize = 32;
/// Han-span lengths scanned for candidates (FR-L3).
pub const CANDIDATE_LEN_RANGE: std::ops::RangeInclusive<usize> = 1..=4;
/// Minimum confidence for heuristic-mode proposals (no KenLM model loaded).
pub const HEURISTIC_MIN_CONFIDENCE: f64 = 0.5;

/// One glossary entry keyed by tone-stripped pinyin.
#[derive(Debug, Clone)]
struct GlossaryEntry {
    surface: String,
    confidence: Option<f64>,
}

/// LM post-correction engine.
pub struct LmCorrector {
    /// Minimum relative PPL improvement required to auto-fix (KenLM mode only).
    ppl_threshold: f64,
    /// Loaded KenLM model handle. Set during construction when a model path is
    /// provided; `None` means the engine operates in heuristic mode.
    model: Option<KenLmModel>,
    /// Jieba segmenter (loaded once at construction; the default dictionary is
    /// embedded in the binary so no runtime file I/O is needed).
    jieba: Jieba,
    /// Glossary index: tone-stripped pinyin → matching glossary terms.
    pinyin_index: HashMap<String, Vec<GlossaryEntry>>,
}

/// A scoring window: a sentence-sized slice plus its byte offset in the original
/// document, so candidate offsets map straight back to source bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Window {
    byte_offset: usize,
    text: String,
}

/// A same-pinyin replacement candidate located in a window.
#[derive(Debug, Clone)]
struct Candidate {
    /// Byte offset of the original span in the *document*.
    byte_offset: usize,
    /// Byte length of the original span.
    original_len: usize,
    /// Surface found in the document.
    original: String,
    /// Proposed same-pinyin replacement.
    replacement: String,
    /// Term confidence from glossary.
    confidence: Option<f64>,
}

impl LmCorrector {
    /// Construct the corrector. Builds the jieba segmenter and a
    /// `(tone_stripped_pinyin → glossary entries)` index from `terms`.
    ///
    /// If `model_path` is `Some`, loads the KenLM model eagerly; returns
    /// `Err(ModelMissing/ModelLoadFailed)` so `--engine lm` exits 1 before
    /// scanning (FR-L6). If `model_path` is `None`, the engine starts in
    /// heuristic mode (spec 055 fallback).
    pub fn new(terms: &[Term], ppl_threshold: f64, model_path: Option<&str>) -> Result<Self> {
        let jieba = Jieba::new();
        let mut pinyin_index: HashMap<String, Vec<GlossaryEntry>> = HashMap::new();

        for t in terms {
            // Only index terms that contain CJK characters.
            if !t.term.contains(|c: char| crate::service::term::lint::is_cjk_ideograph(c)) {
                continue;
            }
            let py = crate::service::term::lint::to_pinyin_no_tone(&t.term);
            if py.is_empty() {
                continue;
            }
            let key = super::rules::tone_stripped(&py);
            pinyin_index
                .entry(key)
                .or_default()
                .push(GlossaryEntry { surface: t.term.clone(), confidence: t.confidence });
        }

        let model = match model_path {
            Some(path) if !path.is_empty() => {
                let p = Path::new(path);
                if !p.exists() {
                    return Err(MfError::ModelMissing {
                        message: format!("LM model not found: {path}"),
                        hint: Some("run `mf term model fetch` to download the default model (057)".to_string()),
                    });
                }
                let m = KenLmModel::load(p).ok_or_else(|| MfError::ModelLoadFailed {
                    message: format!("failed to load LM model: {path}"),
                    hint: Some(
                        "the file may be corrupt; try re-downloading with `mf term model fetch` (057)".to_string(),
                    ),
                })?;
                Some(m)
            }
            _ => None,
        };

        Ok(Self { ppl_threshold, model, jieba, pinyin_index })
    }

    /// Split `content` into scoring windows at sentence terminators and newlines,
    /// preserving each window's starting byte offset. Whitespace-only windows are
    /// dropped. Punctuation deliberately segments windows (spec edge case).
    fn sentence_windows(content: &str) -> Vec<Window> {
        const TERMINATORS: &[char] = &['。', '！', '？', '；', '\n', '.', '!', '?', ';'];
        let mut windows = Vec::new();
        let mut start = 0usize;
        for (idx, ch) in content.char_indices() {
            if TERMINATORS.contains(&ch) {
                let segment = &content[start..idx];
                if !segment.trim().is_empty() {
                    windows.push(Window { byte_offset: start, text: segment.to_string() });
                }
                start = idx + ch.len_utf8();
            }
        }
        // Trailing segment with no terminator.
        if start < content.len() {
            let segment = &content[start..];
            if !segment.trim().is_empty() {
                windows.push(Window { byte_offset: start, text: segment.to_string() });
            }
        }
        windows
    }

    /// Generate same-pinyin replacement candidates for a window using jieba-rs
    /// segmentation + tone-stripped pinyin glossary lookup (FR-L3, spec 055).
    ///
    /// Jieba segments the window text into tokens. Consecutive all-Han tokens
    /// form "Han runs". Within each Han run, a sliding window of 1..=4
    /// characters is scanned and each span's tone-stripped pinyin is looked up
    /// in the glossary index. Matching glossary terms with a different surface
    /// become candidates. Offsets are document-relative
    /// (`window.byte_offset + intra-run offset`).
    fn candidates(&self, window: &Window) -> Vec<Candidate> {
        let mut candidates = Vec::new();
        let tokens = self.jieba.cut(&window.text, false);

        // Build Han runs: concatenations of consecutive all-Han jieba tokens.
        // Each run tracks its byte range within the window so we can compute
        // document-relative offsets.
        let mut han_runs: Vec<(usize, usize)> = Vec::new(); // (byte_start, byte_end) in window
        let mut run_start: Option<usize> = None;
        let mut run_end: usize = 0;

        for token in &tokens {
            let is_han = token.word.chars().all(is_han);
            if is_han {
                if run_start.is_none() {
                    run_start = Some(token.byte_start);
                }
                run_end = token.byte_end;
            } else if let Some(start) = run_start.take() {
                han_runs.push((start, run_end));
            }
        }
        if let Some(start) = run_start {
            han_runs.push((start, run_end));
        }

        // Slide 1..=4 character windows within each Han run and look up pinyin.
        for (run_byte_start, run_byte_end) in &han_runs {
            let run_text = &window.text[*run_byte_start..*run_byte_end];
            let char_offsets: Vec<(usize, char)> = run_text.char_indices().collect();

            let max_len = (*CANDIDATE_LEN_RANGE.end()).min(char_offsets.len());
            let min_len = *CANDIDATE_LEN_RANGE.start();

            let mut i = 0;
            while i < char_offsets.len() {
                let (rel_offset, ch) = char_offsets[i];
                if !is_han(ch) {
                    i += 1;
                    continue;
                }

                for len in min_len..=max_len {
                    let end_idx = i + len;
                    if end_idx > char_offsets.len() {
                        break;
                    }

                    let span_chars: String = char_offsets[i..end_idx].iter().map(|(_, c)| *c).collect();
                    if !span_chars.chars().all(is_han) {
                        continue;
                    }

                    let py = crate::service::term::lint::to_pinyin_no_tone(&span_chars);
                    if py.is_empty() {
                        continue;
                    }
                    let key = super::rules::tone_stripped(&py);
                    if let Some(entries) = self.pinyin_index.get(&key) {
                        let span_byte_end =
                            if end_idx < char_offsets.len() { char_offsets[end_idx].0 } else { run_text.len() };
                        let span_byte_len = span_byte_end - rel_offset;
                        let doc_offset = window.byte_offset + run_byte_start + rel_offset;

                        for entry in entries.iter().take(MAX_CANDIDATES_PER_KEY) {
                            // Skip self-matches.
                            if entry.surface == span_chars {
                                continue;
                            }
                            // Skip when span is a substring of the canonical term.
                            if entry.surface.contains(&span_chars) {
                                continue;
                            }
                            candidates.push(Candidate {
                                byte_offset: doc_offset,
                                original_len: span_byte_len,
                                original: span_chars.clone(),
                                replacement: entry.surface.clone(),
                                confidence: entry.confidence,
                            });
                        }
                    }
                }
                i += 1;
            }
        }

        candidates
    }

    /// Tokenize `text` with jieba, join with spaces for KenLM scoring.
    /// KenLM requires whitespace-separated tokens; raw unsegmented Chinese
    /// would be treated as a single OOV token.
    fn tokenize_for_kenlm(&self, text: &str) -> String {
        let tokens = self.jieba.cut(text, false);
        let words: Vec<&str> = tokens.iter().map(|t| t.word).collect();
        words.join(" ")
    }

    /// Perplexity of `sentence` under the loaded model (lower is better).
    /// Returns `None` when no model is loaded or scoring fails.
    fn perplexity(&self, sentence: &str) -> Option<f64> {
        let model = self.model.as_ref()?;
        let tokenized = self.tokenize_for_kenlm(sentence);
        if tokenized.is_empty() {
            return None;
        }
        let ppl = model.perplexity(&tokenized);
        if ppl.is_finite() && ppl > 0.0 {
            Some(ppl)
        } else {
            None
        }
    }

    /// Check that every token in `sentence` is in-vocabulary under the loaded
    /// model (FR-L4 OOV gate). Returns `true` when no model is loaded (no gate).
    fn all_in_vocab(&self, sentence: &str) -> bool {
        match self.model.as_ref() {
            Some(model) => {
                let tokenized = self.tokenize_for_kenlm(sentence);
                if tokenized.is_empty() {
                    return true;
                }
                model.contains_all(&tokenized)
            }
            None => true,
        }
    }

    /// Relative PPL improvement = `1 - ppl_after / ppl_before` (FR-L4).
    /// Non-finite or non-positive baselines yield `0.0` (no improvement).
    fn relative_improvement(before: f64, after: f64) -> f64 {
        if !before.is_finite() || !after.is_finite() || before <= 0.0 {
            return 0.0;
        }
        1.0 - after / before
    }

    /// Shared candidate gates: reject if the span is protected, overlaps a
    /// declared claim, or overlaps an already-accepted proposal in this batch.
    fn passes_span_gates(
        &self,
        content: &str,
        cand: &Candidate,
        ctx: &CorrectCtx,
        used_spans: &[(usize, usize)],
    ) -> bool {
        if ctx.protected_set.is_protected(content, cand.byte_offset, cand.original_len) {
            return false;
        }
        let span_end = cand.byte_offset + cand.original_len;
        let overlaps = |start: usize, len: usize| {
            let end = start + len;
            (start <= cand.byte_offset && cand.byte_offset < end) || (cand.byte_offset <= start && start < span_end)
        };
        let overlaps_declared = ctx.declared_claims.iter().any(|(_path, off)| overlaps(*off, cand.original_len));
        if overlaps_declared {
            return false;
        }
        !used_spans.iter().any(|(start, end)| overlaps(*start, end - start))
    }
}

impl Corrector for LmCorrector {
    fn engine(&self) -> EngineKind {
        EngineKind::Lm
    }

    fn propose(&self, content: &str, ctx: &CorrectCtx) -> Result<Vec<CorrectionProposal>> {
        if self.model.is_some() {
            // KenLM scoring path (spec 056): sentence windows → perplexity
            // before/after → relative improvement + OOV gates.
            return self.propose_kenlm(content, ctx);
        }

        // Heuristic mode (spec 055): jieba candidates + confidence gating.
        self.propose_heuristic(content, ctx)
    }
}

impl LmCorrector {
    /// Heuristic proposal generation: jieba segmentation → pinyin glossary
    /// lookup → confidence gate. Used when no KenLM model is loaded.
    fn propose_heuristic(&self, content: &str, ctx: &CorrectCtx) -> Result<Vec<CorrectionProposal>> {
        let mut proposals: Vec<CorrectionProposal> = Vec::new();
        let mut used_spans: Vec<(usize, usize)> = Vec::new(); // (start, end) byte offsets

        for window in Self::sentence_windows(content) {
            for cand in self.candidates(&window) {
                if !self.passes_span_gates(content, &cand, ctx, &used_spans) {
                    continue;
                }

                // Confidence heuristic: term confidence ≥ 0.5, or no confidence = eligible.
                let eligible = cand.confidence.unwrap_or(1.0) >= HEURISTIC_MIN_CONFIDENCE;
                if !eligible {
                    continue;
                }

                used_spans.push((cand.byte_offset, cand.byte_offset + cand.original_len));
                proposals.push(CorrectionProposal::heuristic_proposal(
                    cand.byte_offset,
                    cand.original_len,
                    cand.original,
                    cand.replacement,
                    cand.confidence,
                    true,
                    FixKind::Suggested,
                ));
            }
        }

        Ok(proposals)
    }

    /// KenLM-scored proposal generation (spec 056). Called when a KenLM model is
    /// loaded. Scores each sentence window, scores candidate replacements, and
    /// applies the relative-PPL-improvement ≥ threshold gate and no-new-OOV gate
    /// (FR-L4).
    fn propose_kenlm(&self, content: &str, ctx: &CorrectCtx) -> Result<Vec<CorrectionProposal>> {
        // This path only runs when a model is loaded, so the version is real.
        let model_version = KenLmModel::version().to_string();

        let mut proposals = Vec::new();
        let mut used_spans: Vec<(usize, usize)> = Vec::new();

        for window in Self::sentence_windows(content) {
            let before = match self.perplexity(&window.text) {
                Some(p) => p,
                None => continue,
            };

            for cand in self.candidates(&window) {
                if !self.passes_span_gates(content, &cand, ctx, &used_spans) {
                    continue;
                }

                // Build the candidate sentence by splicing the replacement at the
                // candidate's exact span. `cand.byte_offset` is document-relative,
                // so convert it to a window-relative offset. (Using `replacen`
                // here would match the first occurrence of the surface, which may
                // not be this candidate's span when the surface repeats.)
                let rel = cand.byte_offset - window.byte_offset;
                let mut scored = String::with_capacity(window.text.len() - cand.original_len + cand.replacement.len());
                scored.push_str(&window.text[..rel]);
                scored.push_str(&cand.replacement);
                scored.push_str(&window.text[rel + cand.original_len..]);

                let after = match self.perplexity(&scored) {
                    Some(p) => p,
                    None => continue,
                };

                // Gate 1: the replacement must introduce no *new* OOV token
                // (FR-L4). Only the spliced span is new, so check that the
                // replacement's own tokens are all in-vocabulary; a pre-existing
                // OOV token elsewhere in the sentence must not suppress the find.
                if !self.all_in_vocab(&cand.replacement) {
                    continue;
                }

                // Gate 2: relative PPL improvement ≥ threshold (FR-L4).
                let improvement = Self::relative_improvement(before, after);
                let eligible = improvement >= self.ppl_threshold;
                if !eligible {
                    continue;
                }

                used_spans.push((cand.byte_offset, cand.byte_offset + cand.original_len));
                proposals.push(CorrectionProposal::lm_proposal(
                    cand.byte_offset,
                    cand.original_len,
                    cand.original,
                    cand.replacement,
                    model_version.clone(),
                    before,
                    after,
                    improvement,
                    true,
                    FixKind::Required,
                ));
            }
        }

        Ok(proposals)
    }
}

/// Check if a character is a Han/CJK ideograph.
fn is_han(c: char) -> bool {
    crate::service::term::lint::is_cjk_ideograph(c) || ('\u{F900}'..='\u{FAFF}').contains(&c)
    // CJK Compatibility Ideographs
}

#[cfg(test)]
mod tests {
    use super::super::ProtectedSet;
    use super::*;

    // ── sentence windowing ──────────────────────────────────────────────

    #[test]
    fn windows_split_on_terminators_with_offsets() {
        let content = "在线服物上线了。机器人开始工作\n收尾";
        let ws = LmCorrector::sentence_windows(content);
        assert_eq!(ws.len(), 3);
        assert_eq!(ws[0].text, "在线服物上线了");
        assert_eq!(ws[0].byte_offset, 0);
        // Second window starts right after the first terminator '。'.
        assert!(content[ws[1].byte_offset..].starts_with("机器人开始工作"));
        assert_eq!(ws[2].text, "收尾");
    }

    #[test]
    fn windows_drop_empty_segments() {
        assert!(LmCorrector::sentence_windows("。。\n  \n").is_empty());
    }

    // ── relative improvement math ──────────────────────────────────────

    #[test]
    fn relative_improvement_math() {
        // 100 → 70 = 30% improvement.
        assert!((LmCorrector::relative_improvement(100.0, 70.0) - 0.30).abs() < 1e-9);
        // Worse perplexity → negative (not eligible).
        assert!(LmCorrector::relative_improvement(100.0, 120.0) < 0.0);
        // Degenerate baselines → no improvement.
        assert_eq!(LmCorrector::relative_improvement(0.0, 50.0), 0.0);
        assert_eq!(LmCorrector::relative_improvement(f64::NAN, 1.0), 0.0);
    }

    // ── jieba-rs bring-up smoke ────────────────────────────────────────

    #[test]
    fn jieba_segments_chinese_text() {
        let jieba = Jieba::new();
        let tokens = jieba.cut("在线服物上线了", false);
        // jieba should segment into meaningful words.
        assert!(!tokens.is_empty(), "jieba should segment Chinese text");
        let words: Vec<&str> = tokens.iter().map(|t| t.word).collect();
        // "在线" (online) should be recognized.
        assert!(words.contains(&"在线"), "expected 在线 in {:?}", words);
    }

    #[test]
    fn jieba_segments_robot_sentence() {
        let jieba = Jieba::new();
        let tokens = jieba.cut("机器人开始工作", false);
        let words: Vec<&str> = tokens.iter().map(|t| t.word).collect();
        assert!(words.contains(&"机器人"), "expected 机器人 in {:?}", words);
        assert!(words.contains(&"开始"), "expected 开始 in {:?}", words);
    }

    // ── candidate generation ───────────────────────────────────────────

    #[test]
    fn candidates_finds_homophone() {
        let term = Term {
            term: "机器人".into(),
            definition: Some("robot".into()),
            description: None,
            confidence: Some(0.9),
            aliases: vec![],
            tags: vec![],
            corrections: vec![],
        };
        let corrector = LmCorrector::new(&[term], DEFAULT_PPL_THRESHOLD, None).unwrap();

        let window = Window { byte_offset: 0, text: "机器仁开始工作".into() };
        let cands = corrector.candidates(&window);

        // "机器仁" has the same tone-stripped pinyin as "机器人" (ji-qi-ren).
        assert!(!cands.is_empty(), "should find homophone candidate for 机器仁");
        let homophone = cands.iter().find(|c| c.original == "机器仁" && c.replacement == "机器人");
        assert!(homophone.is_some(), "expected 机器仁→机器人 candidate, got {:?}", cands);
    }

    #[test]
    fn candidates_skips_self_match() {
        let term = Term {
            term: "机器人".into(),
            definition: Some("robot".into()),
            description: None,
            confidence: Some(0.9),
            aliases: vec![],
            tags: vec![],
            corrections: vec![],
        };
        let corrector = LmCorrector::new(&[term], DEFAULT_PPL_THRESHOLD, None).unwrap();

        // "机器人" is already correct — no proposal should fire.
        let window = Window { byte_offset: 0, text: "机器人开始工作".into() };
        let cands = corrector.candidates(&window);
        let self_match = cands.iter().find(|c| c.original == "机器人");
        assert!(self_match.is_none(), "should not propose self-match, got {:?}", cands);
    }

    #[test]
    fn candidates_skips_ascii_text() {
        let term = Term {
            term: "机器人".into(),
            definition: Some("robot".into()),
            description: None,
            confidence: Some(0.9),
            aliases: vec![],
            tags: vec![],
            corrections: vec![],
        };
        let corrector = LmCorrector::new(&[term], DEFAULT_PPL_THRESHOLD, None).unwrap();

        // ASCII-only text should yield no candidates.
        let window = Window { byte_offset: 0, text: "hello world".into() };
        let cands = corrector.candidates(&window);
        assert!(cands.is_empty(), "ASCII text should produce no candidates");
    }

    #[test]
    fn candidates_skips_substring_of_canonical() {
        // "网关" is a prefix of "网关API" — should not be "corrected".
        let term = Term {
            term: "网关API".into(),
            definition: Some("Gateway API".into()),
            description: None,
            confidence: Some(0.9),
            aliases: vec![],
            tags: vec![],
            corrections: vec![],
        };
        let corrector = LmCorrector::new(&[term], DEFAULT_PPL_THRESHOLD, None).unwrap();
        let window = Window { byte_offset: 0, text: "网关配置".into() };
        let cands = corrector.candidates(&window);
        // "网关" is a substring of "网关API" so it should not be proposed as a correction.
        let bad = cands.iter().find(|c| c.original == "网关" && c.replacement == "网关API");
        assert!(bad.is_none(), "substring should not be proposed: {:?}", cands);
    }

    // ── heuristic proposal gating ──────────────────────────────────────

    #[test]
    fn heuristic_produces_proposals_for_high_confidence_terms() {
        let term = Term {
            term: "机器人".into(),
            definition: Some("robot".into()),
            description: None,
            confidence: Some(0.9),
            aliases: vec![],
            tags: vec![],
            corrections: vec![],
        };
        let corrector = LmCorrector::new(std::slice::from_ref(&term), DEFAULT_PPL_THRESHOLD, None).unwrap();
        let ctx =
            CorrectCtx { terms: vec![term], declared_claims: Default::default(), protected_set: Default::default() };
        let proposals = corrector.propose("机器仁开始工作", &ctx).unwrap();
        assert!(!proposals.is_empty(), "should find 机器仁→机器人");
        let p = &proposals[0];
        assert_eq!(p.original, "机器仁");
        assert_eq!(p.correct, "机器人");
        assert_eq!(p.engine, EngineKind::Lm);
        assert_eq!(p.model_version.as_deref(), Some("heuristic"));
        assert!(p.ppl_before.is_none());
        assert!(p.ppl_after.is_none());
        assert!(p.replacement_eligible);
    }

    #[test]
    fn heuristic_filters_low_confidence_terms() {
        let term = Term {
            term: "机器人".into(),
            definition: Some("robot".into()),
            description: None,
            confidence: Some(0.3), // below HEURISTIC_MIN_CONFIDENCE (0.5)
            aliases: vec![],
            tags: vec![],
            corrections: vec![],
        };
        let corrector = LmCorrector::new(std::slice::from_ref(&term), DEFAULT_PPL_THRESHOLD, None).unwrap();
        let ctx =
            CorrectCtx { terms: vec![term], declared_claims: Default::default(), protected_set: Default::default() };
        let proposals = corrector.propose("机器仁开始工作", &ctx).unwrap();
        assert!(proposals.is_empty(), "low-confidence term should not produce proposals");
    }

    #[test]
    fn heuristic_no_confidence_defaults_eligible() {
        let term = Term {
            term: "机器人".into(),
            definition: Some("robot".into()),
            description: None,
            confidence: None, // no confidence = eligible by default
            aliases: vec![],
            tags: vec![],
            corrections: vec![],
        };
        let corrector = LmCorrector::new(std::slice::from_ref(&term), DEFAULT_PPL_THRESHOLD, None).unwrap();
        let ctx =
            CorrectCtx { terms: vec![term], declared_claims: Default::default(), protected_set: Default::default() };
        let proposals = corrector.propose("机器仁开始工作", &ctx).unwrap();
        assert!(!proposals.is_empty(), "term without confidence should be eligible");
    }

    #[test]
    fn heuristic_respects_protected_set() {
        let term = Term {
            term: "机器人".into(),
            definition: Some("robot".into()),
            description: None,
            confidence: Some(0.9),
            aliases: vec![],
            tags: vec![],
            corrections: vec![],
        };
        let corrector = LmCorrector::new(std::slice::from_ref(&term), DEFAULT_PPL_THRESHOLD, None).unwrap();
        // "机器仁" is a valid name in this doc → protect it.
        let ctx = CorrectCtx {
            terms: vec![term],
            declared_claims: Default::default(),
            protected_set: {
                let mut set = std::collections::BTreeSet::new();
                set.insert("机器仁".to_string());
                ProtectedSet::new(set)
            },
        };
        let proposals = corrector.propose("机器仁开始工作", &ctx).unwrap();
        assert!(proposals.is_empty(), "protected 机器仁 should not be proposed");
    }

    // ── KenLM scoring (spec 056) ─────────────────────────────────────────

    fn fixture_model_path() -> String {
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest_dir.join("tests/fixtures/asr/tiny_model.arpa").to_string_lossy().to_string()
    }

    #[test]
    fn kenlm_finds_homophone_with_ppl_improvement() {
        let term = Term {
            term: "服务".into(),
            definition: Some("service".into()),
            description: None,
            confidence: Some(0.9),
            aliases: vec![],
            tags: vec![],
            corrections: vec![],
        };
        let corrector =
            LmCorrector::new(std::slice::from_ref(&term), DEFAULT_PPL_THRESHOLD, Some(&fixture_model_path())).unwrap();
        let ctx =
            CorrectCtx { terms: vec![term], declared_claims: Default::default(), protected_set: Default::default() };
        let proposals = corrector.propose("在线服物上线了", &ctx).unwrap();
        assert!(!proposals.is_empty(), "KenLM should find 服物->服务; got {:?}", proposals);
        let p = &proposals[0];
        assert_eq!(p.original, "服物");
        assert_eq!(p.correct, "服务");
        assert_eq!(p.engine, EngineKind::Lm);
        assert!(p.model_version.as_deref().unwrap().starts_with("kenlm-"), "model_version: {:?}", p.model_version);
        assert!(p.ppl_before.unwrap() > 0.0, "ppl_before: {:?}", p.ppl_before);
        assert!(p.ppl_after.unwrap() > 0.0, "ppl_after: {:?}", p.ppl_after);
        assert!(p.ppl_improvement.unwrap() >= DEFAULT_PPL_THRESHOLD, "ppl_improvement: {:?}", p.ppl_improvement);
    }

    #[test]
    fn kenlm_no_finding_for_correct_text() {
        let term = Term {
            term: "服务".into(),
            definition: Some("service".into()),
            description: None,
            confidence: Some(0.9),
            aliases: vec![],
            tags: vec![],
            corrections: vec![],
        };
        let corrector =
            LmCorrector::new(std::slice::from_ref(&term), DEFAULT_PPL_THRESHOLD, Some(&fixture_model_path())).unwrap();
        let ctx =
            CorrectCtx { terms: vec![term], declared_claims: Default::default(), protected_set: Default::default() };
        let proposals = corrector.propose("在线服务上线了", &ctx).unwrap();
        assert!(proposals.is_empty(), "correct text should produce no findings: {:?}", proposals);
    }

    #[test]
    fn kenlm_populates_ppl_fields_on_finding() {
        let term = Term {
            term: "服务".into(),
            definition: Some("service".into()),
            description: None,
            confidence: Some(0.9),
            aliases: vec![],
            tags: vec![],
            corrections: vec![],
        };
        // Use default threshold; fixture model yields very high PPL improvement
        // (near 100%) because it is tiny — the finding should still be emitted.
        let corrector =
            LmCorrector::new(std::slice::from_ref(&term), DEFAULT_PPL_THRESHOLD, Some(&fixture_model_path())).unwrap();
        let ctx =
            CorrectCtx { terms: vec![term], declared_claims: Default::default(), protected_set: Default::default() };
        let proposals = corrector.propose("在线服物上线了", &ctx).unwrap();
        assert!(!proposals.is_empty(), "should find with default threshold");
        // Verify PPL fields are populated (FR-L8).
        let p = &proposals[0];
        assert!(p.ppl_before.unwrap().is_finite() && p.ppl_before.unwrap() > 0.0);
        assert!(p.ppl_after.unwrap().is_finite() && p.ppl_after.unwrap() > 0.0);
        assert!(p.ppl_improvement.unwrap() >= DEFAULT_PPL_THRESHOLD);
        assert!(!p.model_version.as_deref().unwrap().is_empty());
    }
}
