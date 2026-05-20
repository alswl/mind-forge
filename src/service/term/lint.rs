use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::defaults;
use crate::error::{MfError, Result};
use crate::model::index::IndexFile;
use crate::model::term::{TermFinding, TermLintFailure, TermLintReport};
use crate::service::config as config_svc;
use crate::service::index;
use crate::service::util::{atomic_write, canonicalize_within, rel_posix_path};

mod exempt;
mod fix;
mod front_matter;
mod scan;

use self::exempt::strip_exempt_regions;
use self::fix::{apply_fixes, FixSpan};
use self::front_matter::{parse_front_matter_skip_flag, FrontMatterDecision};
use self::scan::{scan_file_for_corrections, InternalFinding};

fn collect_corrections(index: &IndexFile) -> Vec<(String, String, String)> {
    let mut result = Vec::new();
    if let Some(ref terms) = index.terms {
        for t in terms {
            for c in &t.corrections {
                result.push((c.original.clone(), c.correct.clone(), t.term.clone()));
            }
        }
    }
    result
}

fn empty_report(fix: bool, dry_run: bool) -> TermLintReport {
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

    let target_path = project_root.join(file_path);
    if !target_path.exists() {
        return Err(MfError::usage(
            format!("file not found: {file_path}"),
            Some("provide a path relative to the project root".to_string()),
        ));
    }

    let corrections = collect_corrections(&index);
    let mut findings: Vec<TermFinding> = Vec::new();
    let mut internal_findings: Vec<InternalFinding> = Vec::new();
    let mut claimed: BTreeSet<(String, usize)> = BTreeSet::new();
    let mut failures: Vec<TermLintFailure> = Vec::new();

    let rel_path = match rel_posix_path(project_root, &target_path) {
        Ok(r) => r,
        Err(_) => file_path.to_string(),
    };

    let content = match fs::read_to_string(&target_path) {
        Ok(c) => c,
        Err(e) => {
            failures.push(TermLintFailure { path: rel_path.clone(), reason: format!("io error: {e}") });
            return Ok(TermLintReport {
                findings,
                scanned_files: 1,
                skipped_files: vec![],
                fixed_count: 0,
                modified_files: vec![],
                failures,
                would_fix_count: if fix && dry_run { Some(0) } else { None },
            });
        }
    };

    scan_content(&content, None, &corrections, &rel_path, &mut findings, &mut internal_findings, &mut claimed);

    let report = if fix && !dry_run {
        let mut spans: Vec<FixSpan> = internal_findings
            .iter()
            .filter(|ifind| ifind.original != ifind.correct)
            .map(|ifind| FixSpan {
                start: ifind.byte_offset,
                end: ifind.byte_offset + ifind.original_len,
                replacement: ifind.correct.clone(),
            })
            .collect();
        if spans.is_empty() {
            TermLintReport {
                findings,
                scanned_files: 1,
                skipped_files: vec![],
                fixed_count: 0,
                modified_files: vec![],
                failures,
                would_fix_count: None,
            }
        } else {
            spans.sort_by_key(|s| s.start);
            let new_bytes = apply_fixes(content.as_bytes(), &spans);
            let new_content = match String::from_utf8(new_bytes) {
                Ok(c) => c,
                Err(_) => {
                    failures.push(TermLintFailure {
                        path: rel_path.clone(),
                        reason: "non-utf8 content after replacement".to_string(),
                    });
                    return Ok(TermLintReport {
                        findings,
                        scanned_files: 1,
                        skipped_files: vec![],
                        fixed_count: 0,
                        modified_files: vec![],
                        failures,
                        would_fix_count: None,
                    });
                }
            };
            match atomic_write(&target_path, &new_content) {
                Ok(()) => {
                    let modified_paths = vec![rel_path.clone()];
                    TermLintReport {
                        findings,
                        scanned_files: 1,
                        skipped_files: vec![],
                        fixed_count: spans.len() as u64,
                        modified_files: modified_paths,
                        failures,
                        would_fix_count: None,
                    }
                }
                Err(e) => {
                    failures.push(TermLintFailure { path: rel_path, reason: format!("io error: {e}") });
                    TermLintReport {
                        findings,
                        scanned_files: 1,
                        skipped_files: vec![],
                        fixed_count: 0,
                        modified_files: vec![],
                        failures,
                        would_fix_count: None,
                    }
                }
            }
        }
    } else if fix && dry_run {
        let wf = internal_findings.len() as u64;
        TermLintReport {
            findings,
            scanned_files: 1,
            skipped_files: vec![],
            fixed_count: 0,
            modified_files: vec![],
            failures,
            would_fix_count: Some(wf),
        }
    } else {
        TermLintReport {
            findings,
            scanned_files: 1,
            skipped_files: vec![],
            fixed_count: 0,
            modified_files: vec![],
            failures,
            would_fix_count: None,
        }
    };

    Ok(report)
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

    let corrections = collect_corrections(&index);
    let mut findings: Vec<TermFinding> = Vec::new();
    let mut internal_findings: Vec<InternalFinding> = Vec::new();
    let mut scanned_files: u64 = 0;
    let mut skipped_files: Vec<String> = Vec::new();
    let mut failures: Vec<TermLintFailure> = Vec::new();
    let mut claimed: BTreeSet<(String, usize)> = BTreeSet::new();

    let walker = walkdir::WalkDir::new(&docs_dir).into_iter().filter_entry(|e| {
        let name = e.file_name().to_string_lossy();
        !name.starts_with('.') && name != "DS_Store" && name != ".gitkeep"
    });

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().map(|e| e != defaults::MARKDOWN_EXTENSION).unwrap_or(true) {
            continue;
        }

        let rel_path = match rel_posix_path(project_root, path) {
            Ok(r) => r,
            Err(_) => continue,
        };
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
                    &corrections,
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
                    &corrections,
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

    apply_term_fixes(project_root, dry_run, findings, internal_findings, scanned_files, skipped_files, failures)
}

fn scan_content(
    content: &str,
    fm_end: Option<usize>,
    corrections: &[(String, String, String)],
    rel_path: &str,
    findings: &mut Vec<TermFinding>,
    internal_findings: &mut Vec<InternalFinding>,
    claimed: &mut BTreeSet<(String, usize)>,
) {
    let sanitized = strip_exempt_regions(content, fm_end);
    scan_file_for_corrections(content, &sanitized, corrections, rel_path, findings, internal_findings, claimed);
}

fn apply_term_fixes(
    project_root: &Path,
    dry_run: bool,
    findings: Vec<TermFinding>,
    internal_findings: Vec<InternalFinding>,
    scanned_files: u64,
    skipped_files: Vec<String>,
    mut failures: Vec<TermLintFailure>,
) -> Result<TermLintReport> {
    let mut fixed_count: u64 = 0;
    let mut modified_files: Vec<String> = Vec::new();
    let mut would_fix_count: u64 = 0;
    let layout = config_svc::effective_layout(project_root)?;

    let mut by_path: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (idx, ifind) in internal_findings.iter().enumerate() {
        by_path.entry(ifind.path.clone()).or_default().push(idx);
    }

    for (path_rel, indices) in &by_path {
        let full_path = project_root.join(path_rel);
        if canonicalize_within(&project_root.join(&layout.articles), &full_path).is_err() {
            failures.push(TermLintFailure {
                path: path_rel.clone(),
                reason: format!("path escapes project {}/", layout.articles),
            });
            continue;
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
            if ifind.original == ifind.correct {
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
