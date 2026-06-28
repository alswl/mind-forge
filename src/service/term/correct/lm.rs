//! LM engine (spec 055): Jieba-segmented candidate generation with heuristic
//! confidence gating. Full KenLM perplexity scoring is deferred to spec 056.
//!
//! ## Status — heuristic mode (spec 055)
//!
//! Candidate generation via jieba-rs segmentation + tone-stripped pinyin lookup
//! against glossary terms is implemented and verified. When no KenLM model is
//! loaded, proposals are gated by glossary term confidence (≥ 0.5; no confidence
//! field = eligible). Proposals carry `model_version: "heuristic"` and null PPL
//! fields to distinguish them from real KenLM-scored proposals.
//!
//! ## TODO(056) — KenLM scoring path
//!
//! 1. **Model resolution** (`ensure_model`) — configured path → OS cache; return
//!    `Err(ModelMissing/ModelLoadFailed)` so `--engine lm` exits 1 (FR-L6).
//! 2. **Scoring** (`perplexity`) — call `super::kenlm_ffi`; lower is better.
//! 3. **Gates** — relative PPL improvement ≥ threshold AND no new OOV (FR-L4).
//! 4. Once `ensure_model` populates `self.model`, the heuristic path in
//!    `propose()` is skipped and the KenLM scoring path takes over.

use std::collections::HashMap;

use jieba_rs::Jieba;

use crate::error::Result;
use crate::model::term::{EngineKind, FixKind, Term};

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
    /// Glossary terms (protected set + vocabulary hints).
    #[allow(dead_code)]
    terms: Vec<Term>,
    /// Minimum relative PPL improvement required to auto-fix (KenLM mode only).
    #[allow(dead_code)]
    ppl_threshold: f64,
    /// Loaded model handle. `None` until model resolution is implemented (spec 056).
    // TODO(056): replace `()` with `super::kenlm_ffi::Model` once the FFI module
    //            is re-enabled (see correct/mod.rs and build.rs).
    model: Option<()>,
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
    /// Model loading is deferred to [`Self::ensure_model`] (spec 056).
    pub fn new(terms: &[Term], ppl_threshold: f64) -> Self {
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

        Self { terms: terms.to_vec(), ppl_threshold, model: None, jieba, pinyin_index }
    }

    /// Resolve and load the KenLM model (configured path → OS cache).
    ///
    // TODO(056): implement resolution + KenLM load via `super::kenlm_ffi`. On a
    //            missing/corrupt model, return `Err(Error::ModelMissing { .. })`
    //            so `--engine lm` exits 1 before scanning (FR-L6, US2-AC3).
    #[allow(dead_code)]
    fn ensure_model(&mut self) -> Result<()> {
        // Skeleton: no model yet.
        Ok(())
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

    /// Perplexity of `sentence` under the loaded model (lower is better).
    ///
    // TODO(056): call `super::kenlm_ffi` to score. `None` means "cannot score"
    //            (e.g. model absent) → caller skips the window.
    fn perplexity(&self, _sentence: &str) -> Option<f64> {
        None
    }

    /// Relative PPL improvement = `1 - ppl_after / ppl_before` (FR-L4).
    /// Non-finite or non-positive baselines yield `0.0` (no improvement).
    fn relative_improvement(before: f64, after: f64) -> f64 {
        if !before.is_finite() || !after.is_finite() || before <= 0.0 {
            return 0.0;
        }
        1.0 - after / before
    }
}

impl Corrector for LmCorrector {
    fn engine(&self) -> EngineKind {
        EngineKind::Lm
    }

    fn propose(&self, content: &str, ctx: &CorrectCtx) -> Result<Vec<CorrectionProposal>> {
        // TODO(056): when `self.model` is populated by `ensure_model`, take the
        //            KenLM scoring path (sentence windows → perplexity before/after
        //            → relative improvement gate). For now, always use heuristic.
        if self.model.is_some() {
            // KenLM scoring path — unreachable until ensure_model is implemented.
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
                // Respect protected spans.
                if ctx.protected_set.is_protected(content, cand.byte_offset, cand.original_len) {
                    continue;
                }
                // Respect declared claims (same overlap logic as RulesCorrector).
                let overlaps_declared = ctx.declared_claims.iter().any(|(_path, off)| {
                    let decl_end = off + cand.original_len;
                    (*off <= cand.byte_offset && cand.byte_offset < decl_end)
                        || (cand.byte_offset <= *off && *off < cand.byte_offset + cand.original_len)
                });
                if overlaps_declared {
                    continue;
                }
                // Skip if overlaps an already-accepted proposal.
                let overlaps_existing = used_spans.iter().any(|(start, end)| {
                    (*start <= cand.byte_offset && cand.byte_offset < *end)
                        || (cand.byte_offset <= *start && *start < cand.byte_offset + cand.original_len)
                });
                if overlaps_existing {
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

    /// KenLM-scored proposal generation (spec 056). Unreachable until
    /// `ensure_model` loads a real model.
    #[allow(dead_code)]
    fn propose_kenlm(&self, content: &str, ctx: &CorrectCtx) -> Result<Vec<CorrectionProposal>> {
        let mut proposals = Vec::new();
        for window in Self::sentence_windows(content) {
            let before = match self.perplexity(&window.text) {
                Some(p) => p,
                None => continue,
            };

            for cand in self.candidates(&window) {
                // Respect protected spans and declared claims (shared policy).
                if ctx.protected_set.is_protected(content, cand.byte_offset, cand.original_len) {
                    continue;
                }
                if ctx.declared_claims.iter().any(|(_path, off)| *off == cand.byte_offset) {
                    continue;
                }

                // Score the candidate sentence.
                let scored = window.text.replacen(&cand.original, &cand.replacement, 1);
                let after = match self.perplexity(&scored) {
                    Some(p) => p,
                    None => continue,
                };

                let improvement = Self::relative_improvement(before, after);
                // TODO(056): also require no new KenLM OOV token (FR-L4).
                let eligible = improvement >= self.ppl_threshold;
                if !eligible {
                    continue;
                }

                proposals.push(CorrectionProposal::lm_proposal(
                    cand.byte_offset,
                    cand.original_len,
                    cand.original,
                    cand.replacement,
                    // TODO(056): real model version from the manifest.
                    "skeleton".to_string(),
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
        let corrector = LmCorrector::new(&[term], DEFAULT_PPL_THRESHOLD);

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
        let corrector = LmCorrector::new(&[term], DEFAULT_PPL_THRESHOLD);

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
        let corrector = LmCorrector::new(&[term], DEFAULT_PPL_THRESHOLD);

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
        let corrector = LmCorrector::new(&[term], DEFAULT_PPL_THRESHOLD);
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
        let corrector = LmCorrector::new(std::slice::from_ref(&term), DEFAULT_PPL_THRESHOLD);
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
        let corrector = LmCorrector::new(std::slice::from_ref(&term), DEFAULT_PPL_THRESHOLD);
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
        let corrector = LmCorrector::new(std::slice::from_ref(&term), DEFAULT_PPL_THRESHOLD);
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
        let corrector = LmCorrector::new(std::slice::from_ref(&term), DEFAULT_PPL_THRESHOLD);
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
}
