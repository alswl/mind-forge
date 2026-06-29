use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::defaults;
use crate::error::{MfError, Result};
use crate::model::index::IndexFile;
use crate::model::term::{
    Boundary, CandidateTerm, EngineKind, FixKind, Term, TermFinding, TermLintFailure, TermLintReport,
};
use crate::service::config as config_svc;
use crate::service::index;
use crate::service::util::{atomic_write, canonicalize_within, rel_posix_path};

mod exempt;
mod fix;
mod front_matter;
mod pinyin;
mod scan;

use self::exempt::strip_exempt_regions;
pub(crate) use self::fix::{apply_fixes, FixSpan};
pub(crate) use self::front_matter::{parse_front_matter_skip_flag, FrontMatterDecision};
pub(crate) use self::pinyin::to_pinyin_no_tone;
pub(crate) use self::scan::{is_cjk_ideograph, scan_file_for_corrections, InternalFinding};

use super::correct::{Corrector, ProtectedSet};

pub(crate) struct CorrectionEntry {
    pub(crate) original: String,
    pub(crate) correct: String,
    pub(crate) term_name: String,
    pub(crate) description: Option<String>,
    pub(crate) confidence: Option<f64>,
    pub(crate) match_kind: crate::model::term::MatchKind,
    pub(crate) fix_kind: crate::model::term::FixKind,
    pub(crate) boundary: Boundary,
    pub(crate) pinyin: Option<String>,
}

pub(crate) fn collect_corrections(index: &IndexFile) -> Vec<CorrectionEntry> {
    let mut result = Vec::new();
    if let Some(ref terms) = index.terms {
        for t in terms {
            for c in &t.corrections {
                result.push(CorrectionEntry {
                    original: c.original.clone(),
                    correct: c.correct.clone(),
                    term_name: t.term.clone(),
                    description: t.description.clone(),
                    confidence: t.confidence,
                    match_kind: c.r#match,
                    fix_kind: c.fix,
                    boundary: c.boundary,
                    pinyin: c.pinyin.clone(),
                });
            }
        }
    }
    result
}

/// Build a set of original texts that map to more than one term.
pub(crate) fn build_ambiguous_originals(corrections: &[CorrectionEntry]) -> BTreeSet<String> {
    let mut term_counts: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for c in corrections {
        term_counts.entry(&c.original).or_default().insert(&c.term_name);
    }
    term_counts.into_iter().filter(|(_, terms)| terms.len() > 1).map(|(orig, _)| orig.to_string()).collect()
}

/// Build candidate lists for ambiguous originals.
pub(crate) fn build_candidates(
    corrections: &[CorrectionEntry],
    ambiguous: &BTreeSet<String>,
) -> BTreeMap<String, Vec<CandidateTerm>> {
    let mut result: BTreeMap<String, Vec<CandidateTerm>> = BTreeMap::new();
    for c in corrections {
        if ambiguous.contains(&c.original) {
            result.entry(c.original.clone()).or_default().push(CandidateTerm {
                term: c.term_name.clone(),
                correct: c.correct.clone(),
                confidence: c.confidence,
            });
        }
    }
    result
}

/// Merge global terms into a project index for lint fall-through (US3).
/// For each global term whose canonical name does not appear in the project
/// index, append it. Project records shadow global ones by name.
fn merge_global_into_index(project_index: &mut IndexFile, global_terms: Vec<crate::model::term::Term>) {
    let project_terms = project_index.terms.get_or_insert_with(Vec::new);
    let project_names: std::collections::BTreeSet<String> = project_terms.iter().map(|t| t.term.clone()).collect();
    for t in global_terms {
        if !project_names.contains(&t.term) {
            project_terms.push(t);
        }
    }
}

/// Filter `terms` to only those whose canonical name is in `requested`.
/// When `requested` is empty this is a no-op.
/// When non-empty and any requested name is absent from `terms`, returns a usage
/// error listing the unknown names so there is no silent "nothing to fix".
pub fn filter_terms_by_name(terms: &mut Vec<Term>, requested: &[String]) -> Result<()> {
    if requested.is_empty() {
        return Ok(());
    }

    let available: std::collections::BTreeSet<&str> = terms.iter().map(|t| t.term.as_str()).collect();
    let unknown: Vec<&String> = requested.iter().filter(|name| !available.contains(name.as_str())).collect();

    if !unknown.is_empty() {
        let names: Vec<&str> = unknown.iter().map(|s| s.as_str()).collect();
        return Err(MfError::usage(
            format!("unknown term(s): {}", names.join(", ")),
            Some("use `mf term list` to see available terms".to_string()),
        ));
    }

    terms.retain(|t| requested.iter().any(|name| name == &t.term));
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn lint_path_with_global(
    project_root: &Path,
    repo_root: &Path,
    path: &str,
    fix: bool,
    dry_run: bool,
    include_suggested: bool,
    term_filter: &[String],
    engine: EngineKind,
    ppl_threshold: f64,
) -> Result<TermLintReport> {
    let mut index = index::load(project_root)?;
    let global_terms = crate::service::term::global::load_terms(repo_root)?;
    merge_global_into_index(&mut index, global_terms);
    if let Some(ref mut terms) = index.terms {
        filter_terms_by_name(terms, term_filter)?;
    }
    if project_root.join(path).is_dir() {
        lint_dir_with_index(
            &index,
            project_root,
            path,
            "provide a path relative to the project root",
            fix,
            dry_run,
            include_suggested,
            engine,
            ppl_threshold,
        )
    } else {
        lint_single_file_with_index(&index, project_root, path, fix, dry_run, include_suggested, engine, ppl_threshold)
    }
}

#[allow(clippy::too_many_arguments)]
pub fn lint_terms_with_global(
    project_root: &Path,
    repo_root: &Path,
    fix: bool,
    dry_run: bool,
    include_suggested: bool,
    term_filter: &[String],
    engine: EngineKind,
    ppl_threshold: f64,
) -> Result<TermLintReport> {
    let mut index = index::load(project_root)?;
    let global_terms = crate::service::term::global::load_terms(repo_root)?;
    merge_global_into_index(&mut index, global_terms);
    if let Some(ref mut terms) = index.terms {
        filter_terms_by_name(terms, term_filter)?;
    }
    let layout = config_svc::effective_layout(project_root)?;
    let docs_dir = project_root.join(&layout.articles);
    if !docs_dir.exists() {
        return Ok(empty_report(fix, dry_run));
    }
    lint_walk_with_index(
        &index,
        project_root,
        &docs_dir,
        Some(&docs_dir),
        fix,
        dry_run,
        include_suggested,
        engine,
        ppl_threshold,
    )
}

pub(crate) fn empty_report(fix: bool, dry_run: bool) -> TermLintReport {
    TermLintReport {
        findings: vec![],
        scanned_files: 0,
        skipped_files: vec![],
        fixed_count: 0,
        modified_files: vec![],
        failures: vec![],
        would_fix_count: if fix && dry_run { Some(0) } else { None },
        would_apply_count: 0,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn lint_dir_with_index(
    index: &IndexFile,
    base_path: &Path,
    dir_path: &str,
    hint: &str,
    fix: bool,
    dry_run: bool,
    include_suggested: bool,
    engine: EngineKind,
    ppl_threshold: f64,
) -> Result<TermLintReport> {
    let target_dir = base_path.join(dir_path);
    if !target_dir.is_dir() {
        return Err(MfError::usage(format!("not a directory: {dir_path}"), Some(hint.to_string())));
    }
    lint_walk_with_index(
        index,
        base_path,
        &target_dir,
        Some(&target_dir),
        fix,
        dry_run,
        include_suggested,
        engine,
        ppl_threshold,
    )
}

/// Lint a single markdown file against terms in `index`. Used by both project-
/// scoped (`lint_file`) and global (`global::lint_file`) flows; the only
/// difference is the index source and the `base_path` used for atomic writes
/// and rel-path computation.
#[allow(clippy::too_many_arguments)]
pub(crate) fn lint_single_file_with_index(
    index: &IndexFile,
    base_path: &Path,
    file_path: &str,
    fix: bool,
    dry_run: bool,
    include_suggested: bool,
    engine: EngineKind,
    ppl_threshold: f64,
) -> Result<TermLintReport> {
    let target_path = base_path.join(file_path);
    if !target_path.exists() {
        return Err(MfError::usage(
            format!("file not found: {file_path}"),
            Some("provide a path relative to the search root".to_string()),
        ));
    }

    let corrections = collect_corrections(index);
    let ambiguous = build_ambiguous_originals(&corrections);
    let candidates = build_candidates(&corrections, &ambiguous);
    let correction_refs = build_correction_refs(&corrections, &ambiguous, &candidates);
    let mut findings: Vec<TermFinding> = Vec::new();
    let mut internal_findings: Vec<InternalFinding> = Vec::new();
    let mut claimed: BTreeSet<(String, usize)> = BTreeSet::new();
    let mut failures: Vec<TermLintFailure> = Vec::new();

    let rel_path = rel_posix_path(base_path, &target_path).unwrap_or_else(|_| file_path.to_string());

    let content = match fs::read_to_string(&target_path) {
        Ok(c) => c,
        Err(e) => {
            failures.push(TermLintFailure { path: rel_path.clone(), reason: format!("io error: {e}") });
            return Ok(single_file_report(findings, failures, 0, vec![], false, false, 0));
        }
    };

    scan_content(&content, None, &correction_refs, &rel_path, &mut findings, &mut internal_findings, &mut claimed);

    // Run post-correction engine on unclaimed spans.
    if let Some(ref terms) = index.terms {
        run_correction_engine(
            &content,
            &rel_path,
            terms,
            &claimed,
            engine,
            ppl_threshold,
            &mut findings,
            &mut internal_findings,
        )?;
    }

    if !fix {
        return Ok(single_file_report(findings, failures, 0, vec![], false, false, 0));
    }

    let mut would_apply_count: u64 = 0;
    let mut spans: Vec<FixSpan> = Vec::new();
    for ifind in internal_findings.iter() {
        if ifind.original == ifind.correct || ifind.is_ambiguous {
            continue;
        }
        if ifind.fix_kind == FixKind::Suggested && !include_suggested {
            would_apply_count += 1;
            continue;
        }
        spans.push(FixSpan {
            start: ifind.byte_offset,
            end: ifind.byte_offset + ifind.original_len,
            replacement: ifind.correct.clone(),
            correction_order: ifind.yaml_index,
        });
    }

    if dry_run {
        let wf = spans.len() as u64;
        return Ok(TermLintReport {
            findings,
            scanned_files: 1,
            skipped_files: vec![],
            fixed_count: 0,
            modified_files: vec![],
            failures,
            would_fix_count: Some(wf),
            would_apply_count,
        });
    }

    if spans.is_empty() {
        return Ok(single_file_report(findings, failures, 0, vec![], false, false, would_apply_count));
    }

    fix::deduplicate_spans(&mut spans);
    let new_bytes = apply_fixes(content.as_bytes(), &spans);
    let new_content = match String::from_utf8(new_bytes) {
        Ok(c) => c,
        Err(_) => {
            failures.push(TermLintFailure {
                path: rel_path.clone(),
                reason: "non-utf8 content after replacement".to_string(),
            });
            return Ok(single_file_report(findings, failures, 0, vec![], false, false, would_apply_count));
        }
    };

    match atomic_write(&target_path, &new_content) {
        Ok(()) => Ok(single_file_report(
            findings,
            failures,
            spans.len() as u64,
            vec![rel_path],
            false,
            false,
            would_apply_count,
        )),
        Err(e) => {
            failures.push(TermLintFailure { path: rel_path, reason: format!("io error: {e}") });
            Ok(single_file_report(findings, failures, 0, vec![], false, false, would_apply_count))
        }
    }
}

fn single_file_report(
    findings: Vec<TermFinding>,
    failures: Vec<TermLintFailure>,
    fixed_count: u64,
    modified_files: Vec<String>,
    fix: bool,
    dry_run: bool,
    would_apply_count: u64,
) -> TermLintReport {
    TermLintReport {
        findings,
        scanned_files: 1,
        skipped_files: vec![],
        fixed_count,
        modified_files,
        failures,
        would_fix_count: if fix && dry_run { Some(0) } else { None },
        would_apply_count,
    }
}

/// Walk `walk_dir` looking for markdown files and lint them against `index`.
/// `base_path` is the prefix used to derive POSIX-style rel-paths and to
/// resolve atomic writes. `safety_dir`, when `Some`, enables a path-escape
/// check on fix (used by project-scoped lint to confine writes to docs/).
#[allow(clippy::too_many_arguments)]
pub(crate) fn lint_walk_with_index(
    index: &IndexFile,
    base_path: &Path,
    walk_dir: &Path,
    safety_dir: Option<&Path>,
    fix: bool,
    dry_run: bool,
    include_suggested: bool,
    engine: EngineKind,
    ppl_threshold: f64,
) -> Result<TermLintReport> {
    let corrections = collect_corrections(index);
    let ambiguous = build_ambiguous_originals(&corrections);
    let candidates = build_candidates(&corrections, &ambiguous);
    let correction_refs = build_correction_refs(&corrections, &ambiguous, &candidates);
    let mut findings: Vec<TermFinding> = Vec::new();
    let mut internal_findings: Vec<InternalFinding> = Vec::new();
    let mut scanned_files: u64 = 0;
    let mut skipped_files: Vec<String> = Vec::new();
    let mut failures: Vec<TermLintFailure> = Vec::new();
    let mut claimed: BTreeSet<(String, usize)> = BTreeSet::new();

    let walker = walkdir::WalkDir::new(walk_dir).into_iter().filter_entry(|e| {
        let name = e.file_name().to_string_lossy();
        !name.starts_with('.') && name != "DS_Store" && name != ".gitkeep"
    });

    for entry in walker {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().map(|e| e != defaults::MARKDOWN_EXTENSION).unwrap_or(true) {
            continue;
        }

        let Ok(rel_path) = rel_posix_path(base_path, path) else { continue };
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                failures.push(TermLintFailure { path: rel_path, reason: format!("io error: {e}") });
                continue;
            }
        };

        match parse_front_matter_skip_flag(&content) {
            FrontMatterDecision::Skip => skipped_files.push(rel_path),
            FrontMatterDecision::Present { end_byte_offset } => {
                scanned_files += 1;
                scan_content(
                    &content,
                    Some(end_byte_offset),
                    &correction_refs,
                    &rel_path,
                    &mut findings,
                    &mut internal_findings,
                    &mut claimed,
                );
                if let Some(ref terms) = index.terms {
                    run_correction_engine(
                        &content,
                        &rel_path,
                        terms,
                        &claimed,
                        engine,
                        ppl_threshold,
                        &mut findings,
                        &mut internal_findings,
                    )?;
                }
            }
            FrontMatterDecision::None => {
                scanned_files += 1;
                scan_content(
                    &content,
                    None,
                    &correction_refs,
                    &rel_path,
                    &mut findings,
                    &mut internal_findings,
                    &mut claimed,
                );
                if let Some(ref terms) = index.terms {
                    run_correction_engine(
                        &content,
                        &rel_path,
                        terms,
                        &claimed,
                        engine,
                        ppl_threshold,
                        &mut findings,
                        &mut internal_findings,
                    )?;
                }
            }
        }
    }

    findings.sort_by(|a, b| (a.path.as_str(), a.line, a.column).cmp(&(b.path.as_str(), b.line, b.column)));
    skipped_files.sort();
    failures.sort_by(|a, b| a.path.cmp(&b.path));

    if !fix {
        return Ok(TermLintReport {
            findings,
            scanned_files,
            skipped_files,
            fixed_count: 0,
            modified_files: vec![],
            failures,
            would_fix_count: None,
            would_apply_count: 0,
        });
    }

    apply_term_fixes(
        base_path,
        safety_dir,
        dry_run,
        include_suggested,
        findings,
        &internal_findings,
        scanned_files,
        skipped_files,
        failures,
    )
}

pub(crate) fn scan_content(
    content: &str,
    fm_end: Option<usize>,
    correction_refs: &[scan::CorrectionRef<'_>],
    rel_path: &str,
    findings: &mut Vec<TermFinding>,
    internal_findings: &mut Vec<InternalFinding>,
    claimed: &mut BTreeSet<(String, usize)>,
) {
    let sanitized = strip_exempt_regions(content, fm_end);
    scan_file_for_corrections(content, &sanitized, correction_refs, rel_path, findings, internal_findings, claimed);
    pinyin::scan_for_pinyin(content, &sanitized, rel_path, correction_refs, findings, internal_findings, claimed);
}

/// Run the post-correction engine (rules or lm) on `content` after declared
/// corrections have been scanned. Proposals are converted into findings and
/// internal findings, respecting already-claimed offsets.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_correction_engine(
    content: &str,
    rel_path: &str,
    terms: &[crate::model::term::Term],
    claimed: &BTreeSet<(String, usize)>,
    engine: EngineKind,
    ppl_threshold: f64,
    findings: &mut Vec<TermFinding>,
    internal_findings: &mut Vec<InternalFinding>,
) -> Result<()> {
    match engine {
        EngineKind::Rules => {
            let corrector = super::correct::rules::RulesCorrector::new(terms);
            // Build protected set: all canonical term strings
            let protected: ProtectedSet = terms.iter().map(|t| t.term.clone()).collect();
            let declared_claims = claimed.clone();
            let ctx = super::correct::CorrectCtx { terms: terms.to_vec(), declared_claims, protected_set: protected };

            let proposals = corrector.propose(content, &ctx)?;

            for p in proposals {
                let key = (rel_path.to_string(), p.byte_offset);
                if claimed.contains(&key) {
                    continue;
                }
                let (line, col) = scan::byte_offset_to_line_col(content, p.byte_offset);

                findings.push(TermFinding {
                    path: rel_path.to_string(),
                    line,
                    column: col,
                    original: p.original.clone(),
                    correct: p.correct.clone(),
                    term: p.correct.clone(),
                    description: None,
                    confidence: p.confidence,
                    replacement_eligible: p.replacement_eligible,
                    safety_reason: None,
                    candidates: vec![],
                    match_kind: crate::model::term::MatchKind::Word,
                    fix_kind: p.fix_kind,
                    boundary: crate::model::term::Boundary::Loose,
                    boundary_mode: "cjk",
                    engine: Some(EngineKind::Rules),
                    model_version: None,
                    ppl_before: None,
                    ppl_after: None,
                    ppl_improvement: None,
                });

                internal_findings.push(InternalFinding {
                    path: rel_path.to_string(),
                    byte_offset: p.byte_offset,
                    original_len: p.original_len,
                    original: p.original,
                    correct: p.correct,
                    is_ambiguous: false,
                    fix_kind: p.fix_kind,
                    yaml_index: usize::MAX, // generated, not from YAML
                });
            }
            Ok(())
        }
        EngineKind::Lm => {
            // Construct the corrector with an optional model path. When `path` is
            // `Some`, the KenLM model is loaded eagerly; missing/corrupt models
            // fail here before scanning (FR-L6). When `None`, the engine falls
            // back to heuristic mode (spec 055).
            let model_path = std::env::var("MF_LM_MODEL_PATH").ok();
            let corrector = super::correct::lm::LmCorrector::new(terms, ppl_threshold, model_path.as_deref())?;
            let protected: ProtectedSet = terms.iter().map(|t| t.term.clone()).collect();
            let declared_claims = claimed.clone();
            let ctx = super::correct::CorrectCtx { terms: terms.to_vec(), declared_claims, protected_set: protected };

            let proposals = corrector.propose(content, &ctx)?;

            for p in proposals {
                let key = (rel_path.to_string(), p.byte_offset);
                if claimed.contains(&key) {
                    continue;
                }
                let (line, col) = scan::byte_offset_to_line_col(content, p.byte_offset);

                findings.push(TermFinding {
                    path: rel_path.to_string(),
                    line,
                    column: col,
                    original: p.original.clone(),
                    correct: p.correct.clone(),
                    term: p.correct.clone(),
                    description: None,
                    confidence: p.confidence,
                    replacement_eligible: p.replacement_eligible,
                    safety_reason: None,
                    candidates: vec![],
                    match_kind: crate::model::term::MatchKind::Word,
                    fix_kind: p.fix_kind,
                    boundary: crate::model::term::Boundary::Loose,
                    boundary_mode: "cjk",
                    engine: Some(EngineKind::Lm),
                    model_version: p.model_version.clone(),
                    ppl_before: p.ppl_before,
                    ppl_after: p.ppl_after,
                    ppl_improvement: p.ppl_improvement,
                });

                internal_findings.push(InternalFinding {
                    path: rel_path.to_string(),
                    byte_offset: p.byte_offset,
                    original_len: p.original_len,
                    original: p.original,
                    correct: p.correct,
                    is_ambiguous: false,
                    fix_kind: p.fix_kind,
                    yaml_index: usize::MAX, // generated, not from YAML
                });
            }
            Ok(())
        }
    }
}

pub(crate) fn build_correction_refs<'a>(
    corrections: &'a [CorrectionEntry],
    ambiguous: &'a BTreeSet<String>,
    candidates: &'a BTreeMap<String, Vec<CandidateTerm>>,
) -> Vec<scan::CorrectionRef<'a>> {
    corrections
        .iter()
        .enumerate()
        .map(|(yaml_index, c)| {
            let is_ambiguous = ambiguous.contains(&c.original);
            scan::CorrectionRef {
                yaml_index,
                original: &c.original,
                correct: &c.correct,
                term_name: &c.term_name,
                description: c.description.as_deref(),
                confidence: c.confidence,
                is_ambiguous,
                candidates: if is_ambiguous {
                    candidates.get(&c.original).map(|v| v.as_slice()).unwrap_or(&[])
                } else {
                    &[]
                },
                match_kind: c.match_kind,
                fix_kind: c.fix_kind,
                boundary: c.boundary,
                pinyin: c.pinyin.as_deref(),
            }
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn apply_term_fixes(
    base_path: &Path,
    safety_dir: Option<&Path>,
    dry_run: bool,
    include_suggested: bool,
    findings: Vec<TermFinding>,
    internal_findings: &[InternalFinding],
    scanned_files: u64,
    skipped_files: Vec<String>,
    mut failures: Vec<TermLintFailure>,
) -> Result<TermLintReport> {
    let mut fixed_count: u64 = 0;
    let mut modified_files: Vec<String> = Vec::new();
    let mut would_fix_count: u64 = 0;
    let mut would_apply_count: u64 = 0;

    let mut by_path: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (idx, ifind) in internal_findings.iter().enumerate() {
        by_path.entry(ifind.path.clone()).or_default().push(idx);
    }

    for (path_rel, indices) in &by_path {
        let full_path = base_path.join(path_rel);
        if let Some(safety) = safety_dir {
            if canonicalize_within(safety, &full_path).is_err() {
                let safety_label = rel_posix_path(base_path, safety).unwrap_or_else(|_| safety.display().to_string());
                failures
                    .push(TermLintFailure { path: path_rel.clone(), reason: format!("path escapes {safety_label}/") });
                continue;
            }
        }

        let content_orig = match fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(e) => {
                failures.push(TermLintFailure { path: path_rel.clone(), reason: format!("io error: {e}") });
                continue;
            }
        };

        let mut spans = Vec::new();
        let mut per_file_fixed: u64 = 0;
        for &idx in indices {
            let ifind = &internal_findings[idx];
            if ifind.original == ifind.correct || ifind.is_ambiguous {
                continue;
            }
            if ifind.fix_kind == FixKind::Suggested && !include_suggested {
                would_apply_count += 1;
                continue;
            }
            spans.push(FixSpan {
                start: ifind.byte_offset,
                end: ifind.byte_offset + ifind.original_len,
                replacement: ifind.correct.clone(),
                correction_order: ifind.yaml_index,
            });
            per_file_fixed += 1;
        }

        if spans.is_empty() {
            continue;
        }
        if dry_run {
            would_fix_count += per_file_fixed;
            continue;
        }

        fix::deduplicate_spans(&mut spans);
        let new_bytes = apply_fixes(content_orig.as_bytes(), &spans);
        let new_content = String::from_utf8(new_bytes)
            .map_err(|_| MfError::Internal(anyhow::anyhow!("non-utf8 content after replacement")))?;

        match atomic_write(&full_path, &new_content) {
            Ok(()) => {
                fixed_count += per_file_fixed;
                modified_files.push(path_rel.clone());
            }
            Err(e) => failures.push(TermLintFailure { path: path_rel.clone(), reason: format!("io error: {e}") }),
        }
    }

    modified_files.sort();
    failures.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(TermLintReport {
        findings,
        scanned_files,
        skipped_files,
        fixed_count,
        modified_files,
        failures,
        would_fix_count: if dry_run { Some(would_fix_count) } else { None },
        would_apply_count,
    })
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::term::{Correction, Term};

    fn make_term(name: &str) -> Term {
        Term {
            term: name.to_string(),
            definition: None,
            description: None,
            confidence: None,
            aliases: vec![],
            tags: vec![],
            corrections: vec![Correction {
                original: format!("{name}-old"),
                correct: name.to_string(),
                r#match: crate::model::term::MatchKind::Word,
                fix: crate::model::term::FixKind::Required,
                boundary: crate::model::term::Boundary::Loose,
                pinyin: None,
            }],
        }
    }

    #[test]
    fn filter_empty_is_noop() {
        let mut terms = vec![make_term("RAG"), make_term("LLM")];
        filter_terms_by_name(&mut terms, &[]).unwrap();
        assert_eq!(terms.len(), 2);
    }

    #[test]
    fn filter_exact_match_retains_subset() {
        let mut terms = vec![make_term("RAG"), make_term("LLM")];
        filter_terms_by_name(&mut terms, &["RAG".to_string()]).unwrap();
        assert_eq!(terms.len(), 1);
        assert_eq!(terms[0].term, "RAG");
    }

    #[test]
    fn filter_unknown_name_is_error() {
        let mut terms = vec![make_term("RAG")];
        let err = filter_terms_by_name(&mut terms, &["NOPE".to_string()]).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("unknown term"), "expected 'unknown term' in error, got: {msg}");
        assert!(msg.contains("NOPE"), "expected 'NOPE' in error, got: {msg}");
    }

    #[test]
    fn filter_case_sensitive() {
        let mut terms = vec![make_term("RAG")];
        let err = filter_terms_by_name(&mut terms, &["rag".to_string()]).unwrap_err();
        assert!(err.to_string().contains("unknown term"), "case mismatch should be unknown");
    }

    #[test]
    fn filter_multi_term_union() {
        let mut terms = vec![make_term("RAG"), make_term("LLM"), make_term("TPU")];
        filter_terms_by_name(&mut terms, &["RAG".to_string(), "LLM".to_string()]).unwrap();
        assert_eq!(terms.len(), 2);
        let names: std::collections::BTreeSet<&str> = terms.iter().map(|t| t.term.as_str()).collect();
        assert!(names.contains("RAG"));
        assert!(names.contains("LLM"));
    }

    #[test]
    fn filter_mixed_valid_and_unknown_errors() {
        let mut terms = vec![make_term("RAG")];
        let err = filter_terms_by_name(&mut terms, &["RAG".to_string(), "NOPE".to_string()]).unwrap_err();
        assert!(err.to_string().contains("NOPE"));
        assert!(!err.to_string().contains("RAG"), "valid name should not be listed as unknown");
    }
}
