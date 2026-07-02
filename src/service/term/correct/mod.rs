//! ASR post-correction engine framework (spec 055).
//!
//! ## Architecture
//!
//! - `Corrector` trait: engine that scans a document and emits proposals.
//! - `CorrectCtx`: shared context (terms, declared corrections, protected set).
//! - `CorrectionProposal`: a candidate replacement.
//! - `ProtectedSet`: spans the engine may not alter.
//! - Common policy: declared-first, protected-exclusion, deterministic ties.
//!
//! ## Engine
//!
//! - `rules`: glossary homophones via declared corrections + `(pinyin, length)`
//!   index. This is the only in-`mf` engine; open-domain correction is handled
//!   by the driving agent (see spec 058).

pub mod rules;

use std::collections::BTreeSet;

use crate::error::Result;
use crate::model::term::FixKind;

/// A candidate correction produced by the rules engine.
#[derive(Debug, Clone)]
pub struct CorrectionProposal {
    /// UTF-8 byte offset start in original content.
    pub byte_offset: usize,
    /// Non-zero byte length on character boundaries.
    pub original_len: usize,
    /// Surface string found in the document.
    pub original: String,
    /// Proposed replacement.
    pub correct: String,
    /// Optional term confidence.
    pub confidence: Option<f64>,
    /// True when this proposal passes all gates and may be auto-applied.
    pub replacement_eligible: bool,
    /// Fix kind from the source correction.
    pub fix_kind: FixKind,
}

impl CorrectionProposal {
    pub fn rules_proposal(
        byte_offset: usize,
        original_len: usize,
        original: String,
        correct: String,
        confidence: Option<f64>,
        replacement_eligible: bool,
        fix_kind: FixKind,
    ) -> Self {
        Self { byte_offset, original_len, original, correct, confidence, replacement_eligible, fix_kind }
    }
}

/// Shared context for the correction engine during a lint operation.
#[derive(Debug, Clone)]
pub struct CorrectCtx {
    /// Set of byte spans already claimed by declared corrections.
    /// Key: (relative_path, byte_offset, byte_len).
    pub declared_claims: BTreeSet<(String, usize, usize)>,
    /// Protected set: canonical term strings ∪ declared correction target strings.
    pub protected_set: ProtectedSet,
}

/// Set of exact-match document spans the engine may not alter.
#[derive(Debug, Clone, Default)]
pub struct ProtectedSet {
    surfaces: BTreeSet<String>,
}

impl ProtectedSet {
    /// Returns true if the exact string at [offset, offset+len) in `content`
    /// matches any protected surface.
    pub fn is_protected(&self, content: &str, offset: usize, len: usize) -> bool {
        if offset + len > content.len() {
            return true; // invalid span, treat as protected
        }
        self.surfaces.contains(&content[offset..offset + len])
    }
}

impl FromIterator<String> for ProtectedSet {
    fn from_iter<I: IntoIterator<Item = String>>(iter: I) -> Self {
        Self { surfaces: iter.into_iter().collect() }
    }
}

/// Trait for the post-correction engine.
pub trait Corrector {
    /// Scan `content` and produce correction proposals for unclaimed spans.
    /// Proposals must not overlap declared claims or protected spans.
    fn propose(&self, content: &str, ctx: &CorrectCtx) -> Result<Vec<CorrectionProposal>>;
}
