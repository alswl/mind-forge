//! ASR post-correction engine framework (spec 055).
//!
//! ## Architecture
//!
//! - `Corrector` trait: pluggable engine that scans a document and emits proposals.
//! - `CorrectCtx`: shared context (terms, declared corrections, protected set, config).
//! - `CorrectionProposal`: a candidate replacement from any engine.
//! - `ProtectedSet`: spans that neither engine may alter.
//! - Common policy: declared-first, protected-exclusion, deterministic ties.
//!
//! ## Engines
//!
//! - `rules` (default): glossary homophones via `(pinyin, length)` index.
//! - `lm`: Jieba/pinyin candidates scored by KenLM (separate module).

// Types wired in Phase 3+; silence dead_code until then.
#![allow(dead_code)]

pub mod kenlm_ffi;
pub mod lm;
pub mod rules;

use std::collections::BTreeSet;

use crate::error::Result;
use crate::model::term::{EngineKind, FixKind, Term};

/// A candidate correction produced by any engine.
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
    /// Which engine produced this proposal.
    pub engine: EngineKind,
    /// Optional confidence (rules term confidence or normalized LM improvement).
    pub confidence: Option<f64>,
    /// LM model version string (absent for rules).
    pub model_version: Option<String>,
    /// Baseline perplexity (LM only).
    pub ppl_before: Option<f64>,
    /// Perplexity after replacement (LM only).
    pub ppl_after: Option<f64>,
    /// Relative PPL improvement: 1 - ppl_after/ppl_before (LM only).
    pub ppl_improvement: Option<f64>,
    /// True when this proposal passes all engine and common gates.
    pub replacement_eligible: bool,
    /// Fix kind from the source correction.
    pub fix_kind: FixKind,
}

impl CorrectionProposal {
    /// Create a rules proposal (no LM metrics).
    pub fn rules_proposal(
        byte_offset: usize,
        original_len: usize,
        original: String,
        correct: String,
        confidence: Option<f64>,
        replacement_eligible: bool,
        fix_kind: FixKind,
    ) -> Self {
        Self {
            byte_offset,
            original_len,
            original,
            correct,
            engine: EngineKind::Rules,
            confidence,
            model_version: None,
            ppl_before: None,
            ppl_after: None,
            ppl_improvement: None,
            replacement_eligible,
            fix_kind,
        }
    }

    /// Create a heuristic LM proposal (no KenLM scoring; spec 055).
    /// Carries `model_version: "heuristic"` and `ppl_*: null` to distinguish
    /// from real KenLM-scored proposals (spec 056).
    pub fn heuristic_proposal(
        byte_offset: usize,
        original_len: usize,
        original: String,
        correct: String,
        confidence: Option<f64>,
        replacement_eligible: bool,
        fix_kind: FixKind,
    ) -> Self {
        Self {
            byte_offset,
            original_len,
            original,
            correct,
            engine: EngineKind::Lm,
            confidence,
            model_version: Some("heuristic".to_string()),
            ppl_before: None,
            ppl_after: None,
            ppl_improvement: None,
            replacement_eligible,
            fix_kind,
        }
    }

    /// Create an LM proposal with full PPL metrics.
    #[allow(clippy::too_many_arguments)]
    pub fn lm_proposal(
        byte_offset: usize,
        original_len: usize,
        original: String,
        correct: String,
        model_version: String,
        ppl_before: f64,
        ppl_after: f64,
        ppl_improvement: f64,
        replacement_eligible: bool,
        fix_kind: FixKind,
    ) -> Self {
        Self {
            byte_offset,
            original_len,
            original,
            correct,
            engine: EngineKind::Lm,
            confidence: Some(ppl_improvement),
            model_version: Some(model_version),
            ppl_before: Some(ppl_before),
            ppl_after: Some(ppl_after),
            ppl_improvement: Some(ppl_improvement),
            replacement_eligible,
            fix_kind,
        }
    }
}

/// Shared context for correction engines during a lint operation.
#[derive(Debug, Clone)]
pub struct CorrectCtx {
    /// All terms (project + global fallback).
    pub terms: Vec<Term>,
    /// Set of byte spans already claimed by declared corrections.
    /// Key: (relative_path, byte_offset, byte_len).
    pub declared_claims: BTreeSet<(String, usize, usize)>,
    /// Protected set: canonical term strings ∪ declared correction target strings.
    pub protected_set: ProtectedSet,
}

/// Set of exact-match document spans that neither engine may alter.
#[derive(Debug, Clone, Default)]
pub struct ProtectedSet {
    surfaces: BTreeSet<String>,
}

impl ProtectedSet {
    pub fn new(surfaces: BTreeSet<String>) -> Self {
        Self { surfaces }
    }

    /// Returns true if any protected surface appears verbatim in `text`.
    pub fn contains(&self, text: &str) -> bool {
        self.surfaces.contains(text)
    }

    /// Returns true if the exact string at [offset, offset+len) in `content`
    /// matches any protected surface.
    pub fn is_protected(&self, content: &str, offset: usize, len: usize) -> bool {
        if offset + len > content.len() {
            return true; // invalid span, treat as protected
        }
        let candidate = &content[offset..offset + len];
        self.contains(candidate)
    }

    /// Returns the number of protected surfaces.
    pub fn len(&self) -> usize {
        self.surfaces.len()
    }

    /// Returns true when the protected set is empty.
    pub fn is_empty(&self) -> bool {
        self.surfaces.is_empty()
    }
}

impl FromIterator<String> for ProtectedSet {
    fn from_iter<I: IntoIterator<Item = String>>(iter: I) -> Self {
        Self { surfaces: iter.into_iter().collect() }
    }
}

/// Trait for pluggable post-correction engines.
pub trait Corrector {
    /// Return which engine this is.
    fn engine(&self) -> EngineKind;

    /// Scan `content` and produce correction proposals for unclaimed spans.
    /// Proposals must not overlap declared claims or protected spans.
    fn propose(&self, content: &str, ctx: &CorrectCtx) -> Result<Vec<CorrectionProposal>>;
}
