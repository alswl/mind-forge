use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::defaults;
use crate::error::{MfError, Result};
use crate::model::index::IndexFile;
use crate::model::term::{CandidateTerm, TermFinding, TermLintFailure, TermLintReport};
use crate::service::config as config_svc;
use crate::service::index;
use crate::service::util::{atomic_write, canonicalize_within, rel_posix_path};

mod exempt;
mod fix;
mod front_matter;
mod scan;

use self::exempt::strip_exempt_regions;
pub(crate) use self::fix::{apply_fixes, FixSpan};
pub(crate) use self::front_matter::{parse_front_matter_skip_flag, FrontMatterDecision};
pub(crate) use self::scan::{scan_file_for_corrections, InternalFinding};

pub(crate) struct CorrectionEntry {
    pub(crate) original: String,
    pub(crate) correct: String,
    pub(crate) term_name: String,
    pub(crate) description: Option<String>,
    pub(crate) confidence: Option<f64>,
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

pub(crate) fn empty_report(fix: bool, dry_run: bool) -> TermLintReport {
    TermLintReport {
        findings: vec![],
        scanned_files: 0,
        skipped_files: vec![],
        fixed_count: 0,
        modified_files: vec![],
        failures: vec![],
        would_fix_count: if fix && dry_run { Some(0) } else { None },
    }
}

/// Lint a single file for term corrections (FR-027 mind primary form).
pub fn lint_file(project_root: &Path, file_path: &str, fix: bool, dry_run: bool) -> Result<TermLintReport> {
    let index = index::load(project_root)?;
    if index.terms.as_ref().map_or(true, |t| t.is_empty()) {
        return Ok(empty_report(fix, dry_run));
    }
    lint_single_file_with_index(&index, project_root, file_path, fix, dry_run)
}

pub fn lint_terms(project_root: &Path, fix: bool, dry_run: bool) -> Result<TermLintReport> {
    let index = index::load(project_root)?;
    if index.terms.as_ref().map_or(true, |t| t.is_empty()) {
        return Ok(empty_report(fix, dry_run));
    }

    let layout = config_svc::effective_layout(project_root)?;
    let docs_dir = project_root.join(&layout.articles);
    if !docs_dir.exists() {
        return Ok(empty_report(fix, dry_run));
    }

    lint_walk_with_index(&index, project_root, &docs_dir, Some(&docs_dir), fix, dry_run)
}

/// Lint a single markdown file against terms in `index`. Used by both project-
/// scoped (`lint_file`) and global (`global::lint_file`) flows; the only
/// difference is the index source and the `base_path` used for atomic writes
/// and rel-path computation.
pub(crate) fn lint_single_file_with_index(
    index: &IndexFile,
    base_path: &Path,
    file_path: &str,
    fix: bool,
    dry_run: bool,
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
            return Ok(single_file_report(findings, failures, 0, vec![], fix, dry_run));
        }
    };

    scan_content(&content, None, &correction_refs, &rel_path, &mut findings, &mut internal_findings, &mut claimed);

    if !fix {
        return Ok(single_file_report(findings, failures, 0, vec![], false, false));
    }

    let mut spans: Vec<FixSpan> = internal_findings
        .iter()
        .filter(|ifind| ifind.original != ifind.correct && !ifind.is_ambiguous)
        .map(|ifind| FixSpan {
            start: ifind.byte_offset,
            end: ifind.byte_offset + ifind.original_len,
            replacement: ifind.correct.clone(),
        })
        .collect();

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
        });
    }

    if spans.is_empty() {
        return Ok(single_file_report(findings, failures, 0, vec![], false, false));
    }

    spans.sort_by_key(|s| s.start);
    let new_bytes = apply_fixes(content.as_bytes(), &spans);
    let new_content = match String::from_utf8(new_bytes) {
        Ok(c) => c,
        Err(_) => {
            failures.push(TermLintFailure {
                path: rel_path.clone(),
                reason: "non-utf8 content after replacement".to_string(),
            });
            return Ok(single_file_report(findings, failures, 0, vec![], false, false));
        }
    };

    match atomic_write(&target_path, &new_content) {
        Ok(()) => Ok(single_file_report(findings, failures, spans.len() as u64, vec![rel_path], false, false)),
        Err(e) => {
            failures.push(TermLintFailure { path: rel_path, reason: format!("io error: {e}") });
            Ok(single_file_report(findings, failures, 0, vec![], false, false))
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
) -> TermLintReport {
    TermLintReport {
        findings,
        scanned_files: 1,
        skipped_files: vec![],
        fixed_count,
        modified_files,
        failures,
        would_fix_count: if fix && dry_run { Some(0) } else { None },
    }
}

/// Walk `walk_dir` looking for markdown files and lint them against `index`.
/// `base_path` is the prefix used to derive POSIX-style rel-paths and to
/// resolve atomic writes. `safety_dir`, when `Some`, enables a path-escape
/// check on fix (used by project-scoped lint to confine writes to docs/).
pub(crate) fn lint_walk_with_index(
    index: &IndexFile,
    base_path: &Path,
    walk_dir: &Path,
    safety_dir: Option<&Path>,
    fix: bool,
    dry_run: bool,
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
        });
    }

    apply_term_fixes(
        base_path,
        safety_dir,
        dry_run,
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
}

pub(crate) fn build_correction_refs<'a>(
    corrections: &'a [CorrectionEntry],
    ambiguous: &'a BTreeSet<String>,
    candidates: &'a BTreeMap<String, Vec<CandidateTerm>>,
) -> Vec<scan::CorrectionRef<'a>> {
    corrections
        .iter()
        .map(|c| {
            let is_ambiguous = ambiguous.contains(&c.original);
            scan::CorrectionRef {
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
            }
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn apply_term_fixes(
    base_path: &Path,
    safety_dir: Option<&Path>,
    dry_run: bool,
    findings: Vec<TermFinding>,
    internal_findings: &[InternalFinding],
    scanned_files: u64,
    skipped_files: Vec<String>,
    mut failures: Vec<TermLintFailure>,
) -> Result<TermLintReport> {
    let mut fixed_count: u64 = 0;
    let mut modified_files: Vec<String> = Vec::new();
    let mut would_fix_count: u64 = 0;

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
            spans.push(FixSpan {
                start: ifind.byte_offset,
                end: ifind.byte_offset + ifind.original_len,
                replacement: ifind.correct.clone(),
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

        spans.sort_by_key(|s| s.start);
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
    })
}
