//! Deterministic CJK word-segmentation helper backed by jieba-rs.
//!
//! Used by [`super::scan`] to evaluate `WordCheck::Cjk`: a correction fires
//! only when both its left and right edges align with jieba token boundaries.
//!
//! Segmentation is run once per scanned document; the set of token-boundary
//! byte-offsets is cached and reused for all candidate positions (O(n)
//! amortised over a document scan).

use std::collections::BTreeSet;
use std::sync::OnceLock;

use jieba_rs::Jieba;

/// The jieba dictionary is large (hundreds of thousands of entries) and its
/// construction is the dominant cost of segmentation. Build it once per process
/// and share it across all document scans rather than per `segment` call.
fn shared_jieba() -> &'static Jieba {
    static JIEBA: OnceLock<Jieba> = OnceLock::new();
    JIEBA.get_or_init(Jieba::new)
}

/// A lazily-computed set of token-boundary byte-offsets for a document.
///
/// A byte position is a "token boundary" if jieba places a token edge there —
/// i.e., it is either at position 0, at a position where one jieba token ends,
/// or at a position where one jieba token begins.
pub struct JiebaBoundaries {
    /// Sorted, unique set of byte offsets that are valid token boundaries.
    offsets: BTreeSet<usize>,
    /// Exact jieba token spans `(start_byte, token_byte_len)`.
    tokens: BTreeSet<(usize, usize)>,
}

impl JiebaBoundaries {
    /// Run jieba segmentation over `content` and build the boundary set.
    pub fn segment(content: &str) -> Self {
        let cut = shared_jieba().cut(content, true); // true = HMM mode for finer CJK boundaries
        let mut offsets = BTreeSet::new();
        let mut tokens = BTreeSet::new();
        let mut pos: usize = 0;
        for token in &cut {
            offsets.insert(pos);
            tokens.insert((pos, token.len()));
            pos += token.len();
        }
        // Also insert the end-of-document position so span.right == doc_len
        // is always a boundary.
        offsets.insert(pos);
        Self { offsets, tokens }
    }

    /// Returns true if `byte_offset` aligns with a jieba token boundary.
    /// Position 0 and `doc_len` are always boundaries.
    pub fn is_boundary(&self, byte_offset: usize) -> bool {
        self.offsets.contains(&byte_offset)
    }

    /// Returns true if both the left edge (`span_start`) and right edge
    /// (`span_start + span_len`) of a byte-span align with jieba token
    /// boundaries.
    #[inline]
    pub fn span_aligns(&self, span_start: usize, span_len: usize) -> bool {
        self.is_boundary(span_start) && self.is_boundary(span_start + span_len)
    }

    /// Returns true if `content[span_start..span_start + span_len]` is exactly
    /// one jieba token — i.e. jieba emitted this span as a single contiguous
    /// token rather than coincidentally placing token edges on both sides.
    ///
    /// This is the strict check used for the ambiguous short pure-CJK class
    /// (`WordCheck::Cjk` with a ≤2 Han-character span, spec 074 #30): such a
    /// correction fires only when the matched span *is* a token, because
    /// edge-alignment alone fires for spans that merely sit between two
    /// separately-emitted tokens (e.g. 「以可」 in 「以可独立验证」).
    #[inline]
    pub fn is_token(&self, span_start: usize, span_len: usize) -> bool {
        self.tokens.contains(&(span_start, span_len))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_boundary_set() {
        let jb = JiebaBoundaries::segment("实现理想的精神");
        // jieba HMM segmentation: 实现理想 / 的 / 精神
        assert!(jb.is_boundary(0), "0 is always boundary");

        // Verify token boundaries match actual jieba output
        let tokens: Vec<&str> = Jieba::new().cut("实现理想的精神", true);
        // "实现理想" is a single dictionary token; the boundary between
        // 实现 and 理想 is not exposed via jieba's default segmentation.
        let expected = vec!["实现理想", "的", "精神"];
        assert_eq!(tokens, expected, "jieba HMM tokens mismatch");
    }

    #[test]
    fn standalone_word_spans_align() {
        // "时刻" alone should be recognized as a full token
        let jb = JiebaBoundaries::segment("时刻");
        assert!(jb.span_aligns(0, "时刻".len()), "时刻 alone must align");
    }

    #[test]
    fn span_aligns_for_full_token() {
        let jb = JiebaBoundaries::segment("时刻");
        // "时刻" should be a single token
        assert!(jb.span_aligns(0, "时刻".len()));

        // offset 3 is "时" (3 bytes each for CJK chars in UTF-8)
        let shi_bytes = "时".len(); // 3
        assert!(!jb.is_boundary(shi_bytes), "时刻 should not split inside the word");
    }

    #[test]
    fn span_misaligns_for_cross_boundary() {
        // "缩小文件" — jieba HMM: 缩小 / 文件; "小文" crosses the token boundary
        let jb = JiebaBoundaries::segment("缩小文件");
        // "小" starts after "缩" = 3 bytes into text
        let xiao_start = "缩".len(); // 3
        let xiao_wen_len = "小文".len(); // 6
        let tokens: Vec<&str> = Jieba::new().cut("缩小文件", true);
        // Expected HMM tokens: 缩小 / 文件
        assert!(tokens.iter().any(|t| t.contains("缩小")), "expected 缩小 token, got {:?}", tokens);
        assert!(
            !jb.span_aligns(xiao_start, xiao_wen_len),
            "小文 in 缩小文件 should NOT align (crosses token boundary)"
        );
    }

    #[test]
    fn edge_of_document_is_boundary() {
        let jb = JiebaBoundaries::segment("时刻");
        assert!(jb.is_boundary(0));
        assert!(jb.is_boundary("时刻".len()));
    }

    #[test]
    fn empty_document() {
        let jb = JiebaBoundaries::segment("");
        assert!(jb.is_boundary(0));
        assert!(jb.offsets.len() == 1);
        assert!(jb.tokens.is_empty());
    }

    #[test]
    fn is_token_true_for_full_token() {
        let jb = JiebaBoundaries::segment("时刻");
        assert!(jb.is_token(0, "时刻".len()), "时刻 alone must be one exact token");
    }

    #[test]
    fn is_token_false_for_cross_token_span() {
        // 「缩小文件」 segments to 缩小 / 文件; 「小文」 crosses the boundary.
        let jb = JiebaBoundaries::segment("缩小文件");
        assert!(!jb.is_token("缩".len(), "小文".len()), "小文 must not be an exact token in 缩小文件");
    }

    #[test]
    fn is_token_false_for_adjacent_token_pair() {
        // 「以可独立验证」 — 以 and 可 are emitted as separate tokens; the span
        // 「以可」 is edge-aligned but is NOT one exact jieba token.
        let jb = JiebaBoundaries::segment("以可独立验证和回退的方案");
        assert!(
            !jb.is_token(0, "以可".len()),
            "以可 spans two separate jieba tokens in 以可独立验证, must not be an exact token"
        );
    }

    #[test]
    fn shared_instance_is_deterministic_across_calls() {
        // The dictionary is now shared via OnceLock; repeated segmentation of
        // the same and of different documents must stay independent and stable.
        let a1 = JiebaBoundaries::segment("实现理想的精神");
        let a2 = JiebaBoundaries::segment("实现理想的精神");
        assert_eq!(a1.offsets, a2.offsets, "same input must yield identical boundaries");

        // A different document in between must not perturb results.
        let _ = JiebaBoundaries::segment("缩小文件");
        let a3 = JiebaBoundaries::segment("实现理想的精神");
        assert_eq!(a1.offsets, a3.offsets, "shared instance must be stateless across documents");
    }
}
