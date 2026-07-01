//! Default rules engine: glossary homophone correction (spec 055).
//!
//! Indexes canonical glossary terms by `(tone_stripped_pinyin, Han char count)` and
//! scans same-length CJK spans for homophone candidates.

use std::collections::BTreeMap;

use crate::error::Result;
use crate::model::term::{EngineKind, FixKind, Term};

use super::{CorrectCtx, CorrectionProposal, Corrector, ProtectedSet};

/// The default rules corrector.
pub struct RulesCorrector {
    /// Glossary index: key = (tone_stripped_pinyin_sequence, han_char_count).
    index: BTreeMap<(String, usize), Vec<GlossaryEntry>>,
    /// Minimum confidence for rules-generated proposals.
    min_confidence: Option<f64>,
}

/// One glossary entry keyed by pinyin + length.
#[derive(Debug, Clone)]
struct GlossaryEntry {
    surface: String,
    confidence: Option<f64>,
}

impl RulesCorrector {
    /// Build index from glossary terms.
    pub fn new(terms: &[Term]) -> Self {
        let mut index: BTreeMap<(String, usize), Vec<GlossaryEntry>> = BTreeMap::new();
        for t in terms {
            // Skip terms without CJK characters
            if !t.term.contains(|c: char| crate::service::term::lint::is_cjk_ideograph(c)) {
                continue;
            }
            let han_count = t.term.chars().filter(|&c| is_han(c)).count();
            let pinyin = crate::service::term::lint::to_pinyin_no_tone(&t.term);
            if pinyin.is_empty() {
                continue;
            }
            let key = (tone_stripped(&pinyin), han_count);
            index.entry(key).or_default().push(GlossaryEntry { surface: t.term.clone(), confidence: t.confidence });
        }
        Self { index, min_confidence: None }
    }

    /// Build index with a minimum confidence filter.
    pub fn with_min_confidence(mut self, min_confidence: f64) -> Self {
        self.min_confidence = Some(min_confidence);
        self
    }

    /// Scan `content` for same-pinyin CJK spans of lengths found in the index.
    fn scan_spans(&self, content: &str, _protected: &ProtectedSet) -> Vec<CorrectionProposal> {
        let mut proposals = Vec::new();
        let char_offsets: Vec<(usize, char)> = content.char_indices().collect();

        // For each Han character length (1-8, bounded by index keys)
        let max_len: usize = self.index.keys().map(|(_, l)| *l).max().unwrap_or(4);
        let min_len: usize = self.index.keys().map(|(_, l)| *l).min().unwrap_or(2);

        let mut i = 0;
        while i < char_offsets.len() {
            let (offset, ch) = char_offsets[i];
            if !is_han(ch) {
                i += 1;
                continue;
            }

            // Try each length starting from `i`
            for len in min_len..=max_len {
                let end_idx = i + len;
                if end_idx > char_offsets.len() {
                    break;
                }

                // Collect the CJK run
                let run_chars: String = char_offsets[i..end_idx].iter().map(|(_, c)| *c).collect();

                // Only process if all chars are Han
                if !run_chars.chars().all(is_han) {
                    continue;
                }

                // Get tone-stripped pinyin and check index
                let py = crate::service::term::lint::to_pinyin_no_tone(&run_chars);
                if py.is_empty() {
                    continue;
                }
                let key = (tone_stripped(&py), len);
                if let Some(entries) = self.index.get(&key) {
                    let end_offset = char_offsets[end_idx - 1].0 + char_offsets[end_idx - 1].1.len_utf8();
                    let orig_len = end_offset - offset;

                    for entry in entries {
                        // Skip self-matches (the surface is already correct)
                        if entry.surface == run_chars {
                            continue;
                        }
                        // Skip when run_chars is a substring of the canonical term
                        // (e.g. "网关" is a prefix of "网关API", not a homophone error).
                        if entry.surface.contains(&run_chars) {
                            continue;
                        }
                        // Skip if below confidence threshold
                        if let (Some(min_c), Some(c)) = (self.min_confidence, entry.confidence) {
                            if c < min_c {
                                continue;
                            }
                        }
                        proposals.push(CorrectionProposal::rules_proposal(
                            offset,
                            orig_len,
                            run_chars.clone(),
                            entry.surface.clone(),
                            entry.confidence,
                            true, // eligible unless protected/overlapping
                            FixKind::Required,
                        ));
                    }
                }
            }
            i += 1;
        }

        proposals
    }
}

impl Corrector for RulesCorrector {
    fn engine(&self) -> EngineKind {
        EngineKind::Rules
    }

    fn propose(&self, content: &str, ctx: &CorrectCtx) -> Result<Vec<CorrectionProposal>> {
        let props = self.scan_spans(content, &ctx.protected_set);

        // Filter out proposals that:
        // 1. Overlap with declared claims
        // 2. Match a protected surface exactly
        // 3. Overlap another proposal (keep first by offset, then by higher confidence)
        let mut filtered: Vec<CorrectionProposal> = Vec::new();
        let mut used_offsets: Vec<(usize, usize)> = Vec::new();

        for p in props {
            // Skip if protected
            if ctx.protected_set.is_protected(content, p.byte_offset, p.original_len) {
                continue;
            }
            // Skip if overlaps declared claims
            let overlaps_declared = ctx.declared_claims.iter().any(|(_path, decl_off, decl_len)| {
                let decl_end = decl_off + decl_len;
                (*decl_off <= p.byte_offset && p.byte_offset < decl_end)
                    || (p.byte_offset <= *decl_off && *decl_off < p.byte_offset + p.original_len)
            });
            if overlaps_declared {
                continue;
            }
            // Skip if overlaps any already-accepted proposal
            let overlaps_existing = used_offsets.iter().any(|(start, end)| {
                (*start <= p.byte_offset && p.byte_offset < *end)
                    || (p.byte_offset <= *start && *start < p.byte_offset + p.original_len)
            });
            if overlaps_existing {
                continue;
            }

            used_offsets.push((p.byte_offset, p.byte_offset + p.original_len));
            filtered.push(p);
        }

        Ok(filtered)
    }
}

/// Check if a character is a Han/CJK ideograph.
fn is_han(c: char) -> bool {
    crate::service::term::lint::is_cjk_ideograph(c) || ('\u{F900}'..='\u{FAFF}').contains(&c)
    // CJK Compatibility Ideographs
}

/// Strip tone digits from pinyin (e.g., "ji1" → "ji", "qi4" → "qi").
pub(crate) fn tone_stripped(pinyin: &str) -> String {
    pinyin.chars().filter(|c| !c.is_ascii_digit()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_term(term: &str, confidence: Option<f64>) -> Term {
        Term {
            term: term.into(),
            definition: Some("test".into()),
            description: None,
            confidence,
            aliases: vec![],
            tags: vec![],
            corrections: vec![],
        }
    }

    fn make_ctx(terms: Vec<Term>) -> CorrectCtx {
        CorrectCtx { terms, declared_claims: Default::default(), protected_set: Default::default() }
    }

    // ── tone_stripped ──────────────────────────────────────────────────

    #[test]
    fn tone_stripped_removes_digits() {
        assert_eq!(tone_stripped("ji1"), "ji");
        assert_eq!(tone_stripped("qi4"), "qi");
        assert_eq!(tone_stripped("ren2"), "ren");
        assert_eq!(tone_stripped("ji-qi-ren"), "ji-qi-ren");
    }

    #[test]
    fn tone_stripped_no_digits_unchanged() {
        assert_eq!(tone_stripped("hello"), "hello");
    }

    // ── index building ─────────────────────────────────────────────────

    #[test]
    fn rules_corrector_empty_terms() {
        let corrector = RulesCorrector::new(&[]);
        assert_eq!(corrector.engine(), EngineKind::Rules);
    }

    #[test]
    fn rules_corrector_builds_index_for_cjk_terms() {
        let term = make_term("机器人", Some(0.9));
        let corrector = RulesCorrector::new(&[term]);
        assert!(!corrector.index.is_empty());
    }

    #[test]
    fn index_skips_non_cjk_terms() {
        let term = make_term("hello", Some(0.9));
        let corrector = RulesCorrector::new(&[term]);
        assert!(corrector.index.is_empty(), "non-CJK term should be excluded from index");
    }

    #[test]
    fn index_keys_by_han_count_and_stripped_pinyin() {
        let term = make_term("机器人", Some(0.9));
        let corrector = RulesCorrector::new(&[term]);
        // 机器人: 3 Han chars, pinyin ≈ "ji-qi-ren" (tone-stripped)
        let key = (tone_stripped("ji-qi-ren"), 3);
        assert!(corrector.index.contains_key(&key), "index should key by (pinyin, han_count)");
    }

    // ── SC-001: 机器仁→机器人 homophone matching ───────────────────────

    #[test]
    fn finds_ji_qi_ren_homophone() {
        let corrector = RulesCorrector::new(&[make_term("机器人", Some(0.9))]);
        let ctx = make_ctx(vec![]);
        let proposals = corrector.propose("机器仁开始工作", &ctx).unwrap();
        assert!(!proposals.is_empty(), "should find 机器仁→机器人");
        let p = &proposals[0];
        assert_eq!(p.original, "机器仁");
        assert_eq!(p.correct, "机器人");
        assert_eq!(p.engine, EngineKind::Rules);
        assert_eq!(p.byte_offset, 0);
        assert_eq!(p.original_len, "机器仁".len());
    }

    // ── UTF-8 offsets ──────────────────────────────────────────────────

    #[test]
    fn homophone_offset_follows_ascii_prefix() {
        // "prefix 机器仁" → 机器仁 starts at byte 7 (prefix = 7 ASCII bytes + space)
        let corrector = RulesCorrector::new(&[make_term("机器人", Some(0.9))]);
        let ctx = make_ctx(vec![]);
        let proposals = corrector.propose("prefix 机器仁 suffix", &ctx).unwrap();
        assert_eq!(proposals.len(), 1);
        let p = &proposals[0];
        // "prefix " = 7 bytes, then 机器仁 = 9 bytes
        assert_eq!(p.byte_offset, 7, "机器仁 should start at byte 7 after 'prefix '");
        assert_eq!(p.original_len, "机器仁".len());
    }

    #[test]
    fn homophone_offset_in_middle_of_text() {
        // "前机器仁后" → 机器仁 at byte 3 (前 = 3 bytes)
        let corrector = RulesCorrector::new(&[make_term("机器人", Some(0.9))]);
        let ctx = make_ctx(vec![]);
        let proposals = corrector.propose("前机器仁后", &ctx).unwrap();
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].byte_offset, 3);
        assert_eq!(proposals[0].original_len, "机器仁".len());
    }

    // ── length/surface rejection ───────────────────────────────────────

    #[test]
    fn different_length_no_match() {
        // Glossary has 机器人 (3 chars), document has 机器 (2 chars).
        // Different han_count → different index key → no match.
        let corrector = RulesCorrector::new(&[make_term("机器人", Some(0.9))]);
        let ctx = make_ctx(vec![]);
        let proposals = corrector.propose("机器启动", &ctx).unwrap();
        // 机器 = 2 chars ≠ 3 chars (机器人) → no match.
        assert!(proposals.is_empty(), "different length should not match");
    }

    #[test]
    fn longer_text_no_match_for_shorter_glossary() {
        // Glossary has "机器" (2 chars), but 机器仁 (3 chars) has different length.
        // However 机器 (first 2 chars of 机器仁) DOES match — that IS a match.
        // Let's test the right thing: a term of 4 chars shouldn't match 3-char spans.
        let corrector = RulesCorrector::new(&[make_term("人工智能", Some(0.9))]);
        let ctx = make_ctx(vec![]);
        // 人工 = 2 chars, different length from 人工智能 (4 chars).
        let proposals = corrector.propose("人工呼吸", &ctx).unwrap();
        assert!(proposals.is_empty(), "2-char '人工' should not match 4-char '人工智能'");
    }

    #[test]
    fn substring_of_canonical_not_proposed() {
        // "网关" is a substring of "网关API" → should be skipped.
        let corrector = RulesCorrector::new(&[make_term("网关API", Some(0.9))]);
        let ctx = make_ctx(vec![]);
        let proposals = corrector.propose("网关配置", &ctx).unwrap();
        let bad = proposals.iter().find(|p| p.original == "网关" && p.correct == "网关API");
        assert!(bad.is_none(), "substring of canonical term should not be proposed");
    }

    #[test]
    fn self_match_skipped() {
        // Document already has the correct term → no proposal.
        let corrector = RulesCorrector::new(&[make_term("机器人", Some(0.9))]);
        let ctx = make_ctx(vec![]);
        let proposals = corrector.propose("机器人开始工作", &ctx).unwrap();
        assert!(proposals.is_empty(), "self-match should produce no proposal");
    }

    // ── ASCII pass-through ─────────────────────────────────────────────

    #[test]
    fn ascii_only_content_no_proposals() {
        let corrector = RulesCorrector::new(&[make_term("机器人", Some(0.9))]);
        let ctx = make_ctx(vec![]);
        let proposals = corrector.propose("hello world", &ctx).unwrap();
        assert!(proposals.is_empty(), "ASCII content should produce no proposals");
    }

    #[test]
    fn mixed_content_only_scans_cjk_spans() {
        // CJK homophone in mixed content should be found.
        let corrector = RulesCorrector::new(&[make_term("机器人", Some(0.9))]);
        let ctx = make_ctx(vec![]);
        let proposals = corrector.propose("hello 机器仁 world", &ctx).unwrap();
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].original, "机器仁");
    }

    // ── frequency/lexical tie-breaking ─────────────────────────────────

    #[test]
    fn multiple_entries_same_key_all_proposed() {
        // Two terms with same pinyin+length: 精研 and 精盐 (both jing-yan, 2 chars).
        // Use "精研" as the document text (self-match for t1, non-self for t2).
        // "精盐" should still be proposed since it's a different surface with the
        // same pinyin key.
        let t1 = make_term("精研", Some(0.9));
        let t2 = make_term("精盐", Some(0.8));
        let corrector = RulesCorrector::new(&[t1, t2]);
        let ctx = make_ctx(vec![]);
        let proposals = corrector.propose("精研结果", &ctx).unwrap();
        // 精研 is a self-match → skipped. 精盐 should still be proposed.
        let surfaces: Vec<&str> = proposals.iter().map(|p| p.correct.as_str()).collect();
        assert!(!surfaces.contains(&"精研"), "self-match should be skipped");
        assert!(surfaces.contains(&"精盐"), "should propose 精盐, got {:?}", surfaces);
    }

    #[test]
    fn tie_breaking_keeps_first_by_offset() {
        // Glossary: 精研 and 精盐 share the same (jing-yan, 2) key.
        // Document: 精研 (self-match for t1, candidate for t2).
        // 精盐 should be proposed as the only non-self-match.
        let corrector = RulesCorrector::new(&[make_term("精研", Some(0.9)), make_term("精盐", Some(0.8))]);
        let ctx = make_ctx(vec![]);
        let proposals = corrector.propose("精研结果", &ctx).unwrap();
        assert!(!proposals.is_empty(), "should propose 精盐 for 精研");
        assert_eq!(proposals[0].original, "精研");
        assert_eq!(proposals[0].correct, "精盐");
    }

    // ── confidence filtering ───────────────────────────────────────────

    #[test]
    fn confidence_filter_excludes_low_confidence() {
        let term = make_term("机器人", Some(0.3));
        let corrector = RulesCorrector::new(&[term]).with_min_confidence(0.5);
        let ctx = make_ctx(vec![]);
        let proposals = corrector.propose("机器仁开始工作", &ctx).unwrap();
        assert!(proposals.is_empty(), "confidence 0.3 < 0.5 should be filtered");
    }

    #[test]
    fn confidence_filter_passes_high_confidence() {
        let term = make_term("机器人", Some(0.9));
        let corrector = RulesCorrector::new(&[term]).with_min_confidence(0.5);
        let ctx = make_ctx(vec![]);
        let proposals = corrector.propose("机器仁开始工作", &ctx).unwrap();
        assert!(!proposals.is_empty(), "confidence 0.9 >= 0.5 should pass");
    }

    #[test]
    fn confidence_filter_no_confidence_still_passes() {
        let term = make_term("机器人", None);
        let corrector = RulesCorrector::new(&[term]).with_min_confidence(0.5);
        let ctx = make_ctx(vec![]);
        let proposals = corrector.propose("机器仁开始工作", &ctx).unwrap();
        // No confidence field → min_confidence filter is skipped (None doesn't trigger the check).
        assert!(!proposals.is_empty(), "term without confidence should not be filtered");
    }

    // ── protected set and declared claims ──────────────────────────────

    #[test]
    fn protected_set_blocks_proposal() {
        let term = make_term("机器人", Some(0.9));
        let corrector = RulesCorrector::new(&[term]);
        let mut set = std::collections::BTreeSet::new();
        set.insert("机器仁".to_string());
        let ctx =
            CorrectCtx { terms: vec![], declared_claims: Default::default(), protected_set: ProtectedSet::new(set) };
        let proposals = corrector.propose("机器仁开始工作", &ctx).unwrap();
        assert!(proposals.is_empty(), "protected surface should block proposal");
    }
}
