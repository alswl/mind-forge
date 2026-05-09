// Term service - implemented in 012-term-core

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::error::{MfError, Result};
use crate::model::index::IndexFile;
use crate::model::term::{Correction, Term, TermFinding, TermLintFailure, TermLintReport};
use crate::service::util::{
    atomic_write, canonicalize_within, rel_posix_path, validate_schema_version,
};

// ── Index helpers (T006) ─────────────────────────────────────────────────────

pub fn load_index(project_root: &Path) -> Result<IndexFile> {
    let path = project_root.join("mind-index.yaml");
    if !path.exists() {
        return Ok(IndexFile::create_default());
    }
    let content = fs::read_to_string(&path)
        .map_err(|e| MfError::usage(format!("cannot read mind-index.yaml: {e}"), None::<String>))?;
    let index: IndexFile = serde_yaml::from_str(&content).map_err(|e| MfError::ParseError {
        kind: "yaml".to_string(),
        path: path.clone(),
        detail: e.to_string(),
    })?;
    validate_schema_version(&index.schema_version, &path)?;
    Ok(index)
}

pub fn save_index(project_root: &Path, index: &IndexFile) -> Result<()> {
    let path = project_root.join("mind-index.yaml");
    let yaml = serde_yaml::to_string(index)
        .map_err(|e| MfError::Io(std::io::Error::other(format!("cannot serialize index: {e}",))))?;
    atomic_write(&path, &yaml)
}

// ── Term lookup (T007) ───────────────────────────────────────────────────────

pub(crate) fn find_term_by_correct(index: &IndexFile, correct: &str) -> Result<usize> {
    let terms = index.terms.as_deref().unwrap_or_default();
    let matches: Vec<usize> = terms
        .iter()
        .enumerate()
        .filter(|(_, t)| t.term == correct || t.aliases.iter().any(|a| a == correct))
        .map(|(i, _)| i)
        .collect();

    match matches.len() {
        0 => Err(MfError::usage(
            format!("no term registers '{correct}' as its main name or alias"),
            Some(format!(
                "register first with 'mf term new {correct}' or 'mf term fix --alias {correct}'"
            )),
        )),
        1 => Ok(matches[0]),
        _ => {
            let candidates: Vec<&str> = matches.iter().map(|&i| terms[i].term.as_str()).collect();
            Err(MfError::usage(
                format!("multiple terms claim '{correct}': {candidates:?}"),
                Some("disambiguate by editing 'mf term fix' for the chosen term".to_string()),
            ))
        }
    }
}

// ── Helper: dedup preserving first occurrence order ──────────────────────────

fn dedup_preserve_first(items: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for item in items {
        if seen.insert(item.clone()) {
            result.push(item.clone());
        }
    }
    result
}

// ── Sort terms by `term` field ───────────────────────────────────────────────

fn sort_terms_by_name(terms: &mut [Term]) {
    terms.sort_by(|a, b| a.term.cmp(&b.term));
}

// ── US1: new_term (T016) ────────────────────────────────────────────────────

pub fn new_term(
    project_root: &Path,
    term: &str,
    definition: Option<&str>,
    aliases: &[String],
    tags: &[String],
) -> Result<Term> {
    if term.trim().is_empty() {
        return Err(MfError::usage("term name cannot be empty", None));
    }

    let mut index = load_index(project_root)?;
    let terms = index.terms.get_or_insert_with(Vec::new);

    // Check for duplicate (strict case-sensitive)
    if terms.iter().any(|t| t.term == term) {
        return Err(MfError::usage(
            format!("term '{term}' already exists"),
            Some("use 'mf term fix' to modify the existing term".to_string()),
        ));
    }

    // Dedup before checking alias conflicts
    let aliases = dedup_preserve_first(aliases);
    let tags = dedup_preserve_first(tags);

    // Check alias uniqueness across existing terms
    for alias in &aliases {
        for t in terms.iter() {
            if t.term == *alias || t.aliases.iter().any(|a| a == alias) {
                return Err(MfError::usage(
                    format!("alias '{alias}' conflicts with existing term '{}'", t.term),
                    None,
                ));
            }
        }
    }

    let new_entry = Term {
        term: term.to_string(),
        definition: definition.and_then(|s| if s.is_empty() { None } else { Some(s.to_string()) }),
        aliases,
        tags,
        corrections: vec![],
    };

    terms.push(new_entry.clone());
    sort_terms_by_name(terms);
    save_index(project_root, &index)?;

    Ok(new_entry)
}

// ── US2: list_terms (T020) ──────────────────────────────────────────────────

pub fn list_terms(project_root: &Path, filter: Option<&str>) -> Result<Vec<Term>> {
    let index = load_index(project_root)?;
    let mut terms = match index.terms {
        Some(t) => t,
        None => return Ok(vec![]),
    };

    if let Some(f) = filter {
        let lower = f.to_lowercase();
        terms.retain(|t| {
            t.term.to_lowercase().contains(&lower)
                || t.aliases.iter().any(|a| a.to_lowercase().contains(&lower))
                || t.tags.iter().any(|tag| tag.to_lowercase().contains(&lower))
        });
    }

    terms.sort_by(|a, b| a.term.cmp(&b.term));
    Ok(terms)
}

// ── US3 lint helpers (T024-T025) ─────────────────────────────────────────────

fn byte_offset_to_line_col(content: &str, byte_offset: usize) -> (u32, u32) {
    let mut line: u32 = 1;
    let mut col: u32 = 1;
    for (i, c) in content.char_indices() {
        if i >= byte_offset {
            break;
        }
        if c == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

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

// ── Internal finding with byte offset (for --fix) ────────────────────────────

struct InternalFinding {
    path: String,
    byte_offset: usize,
    original_len: usize,
    original: String,
    correct: String,
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

// ── US3: lint_terms (T026) — read-only + fix skeleton ───────────────────────
// ── US4: fix branch (T032) ──────────────────────────────────────────────────

pub fn lint_terms(project_root: &Path, fix: bool, dry_run: bool) -> Result<TermLintReport> {
    let index = load_index(project_root)?;

    // Early return if no terms registered
    if index.terms.as_ref().map_or(true, |t| t.is_empty()) {
        return Ok(empty_report(fix, dry_run));
    }

    let corrections = collect_corrections(&index);

    let docs_dir = project_root.join("docs");

    let mut findings: Vec<TermFinding> = Vec::new();
    let mut internal_findings: Vec<InternalFinding> = Vec::new();
    let mut scanned_files: u64 = 0;
    let mut skipped_files: Vec<String> = Vec::new();
    let mut failures: Vec<TermLintFailure> = Vec::new();

    // Walk docs/ directory
    if !docs_dir.exists() {
        return Ok(empty_report(fix, dry_run));
    }

    let walker = walkdir::WalkDir::new(&docs_dir).into_iter().filter_entry(|e| {
        let name = e.file_name().to_string_lossy();
        !name.starts_with('.') && name != "DS_Store" && name != ".gitkeep"
    });

    // Prepare for dedup: track (path_rel, byte_offset) claimed
    let mut claimed: BTreeSet<(String, usize)> = BTreeSet::new();

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().map(|e| e != "md").unwrap_or(true) {
            continue;
        }

        // Compute relative path
        let rel_path = match rel_posix_path(project_root, path) {
            Ok(r) => r,
            Err(_) => continue,
        };

        // Read file content
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                failures.push(TermLintFailure { path: rel_path, reason: format!("io error: {e}") });
                continue;
            }
        };

        // Parse front-matter skip flag
        let fm_decision = parse_front_matter_skip_flag(&content);
        match fm_decision {
            FrontMatterDecision::Skip => {
                skipped_files.push(rel_path);
                continue;
            }
            FrontMatterDecision::Present { end_byte_offset } => {
                scanned_files += 1;
                let sanitized = strip_exempt_regions(&content, Some(end_byte_offset));
                scan_file_for_corrections(
                    &content,
                    &sanitized,
                    &corrections,
                    &rel_path,
                    &mut findings,
                    &mut internal_findings,
                    &mut claimed,
                );
            }
            FrontMatterDecision::None => {
                scanned_files += 1;
                let sanitized = strip_exempt_regions(&content, None);
                scan_file_for_corrections(
                    &content,
                    &sanitized,
                    &corrections,
                    &rel_path,
                    &mut findings,
                    &mut internal_findings,
                    &mut claimed,
                );
            }
        }
    }

    // Sort findings as required
    findings.sort_by(|a, b| {
        (a.path.as_str(), a.line, a.column).cmp(&(b.path.as_str(), b.line, b.column))
    });
    skipped_files.sort();
    failures.sort_by(|a, b| a.path.cmp(&b.path));

    if !fix {
        return Ok(TermLintReport {
            findings,
            scanned_files,
            skipped_files: skipped_files.clone(),
            fixed_count: 0,
            modified_files: vec![],
            failures,
            would_fix_count: None,
        });
    }

    // ── --fix branch (US4 / T032) ──
    let mut fixed_count: u64 = 0;
    let mut modified_files: Vec<String> = Vec::new();
    let mut would_fix_count: u64 = 0;

    // Group internal findings by path
    let mut by_path: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (idx, ifind) in internal_findings.iter().enumerate() {
        by_path.entry(ifind.path.clone()).or_default().push(idx);
    }

    for (path_rel, indices) in &by_path {
        // Verify the file is within project/docs/ via canonicalize_within
        let full_path = project_root.join(path_rel);
        if let Err(_e) = canonicalize_within(&project_root.join("docs"), &full_path) {
            failures.push(TermLintFailure {
                path: path_rel.clone(),
                reason: "path escapes project docs/".to_string(),
            });
            continue;
        }

        // Read original content again for fix
        let content_orig = match fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(e) => {
                failures.push(TermLintFailure {
                    path: path_rel.clone(),
                    reason: format!("io error: {e}"),
                });
                continue;
            }
        };

        // Build FixSpans, filtering out original==correct
        let mut spans: Vec<FixSpan> = Vec::new();
        let mut per_file_fixed: u64 = 0;
        for &idx in indices {
            let ifind = &internal_findings[idx];
            if ifind.original == ifind.correct {
                continue; // FR-303 zero-op
            }
            spans.push(FixSpan {
                start: ifind.byte_offset,
                end: ifind.byte_offset + ifind.original_len,
                replacement: ifind.correct.clone(),
            });
            per_file_fixed += 1;
        }

        if spans.is_empty() {
            // No actual fixes needed for this file (all were zero-ops)
            continue;
        }

        if dry_run {
            would_fix_count += per_file_fixed;
            continue;
        }

        // Apply fixes and write back
        spans.sort_by_key(|s| s.start);
        let new_bytes = apply_fixes(content_orig.as_bytes(), &spans);
        let new_content = String::from_utf8(new_bytes)
            .map_err(|_| MfError::usage("non-utf8 content after replacement", None::<String>))?;

        match atomic_write(&full_path, &new_content) {
            Ok(()) => {
                fixed_count += per_file_fixed;
                modified_files.push(path_rel.clone());
            }
            Err(e) => {
                failures.push(TermLintFailure {
                    path: path_rel.clone(),
                    reason: format!("io error: {e}"),
                });
            }
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

// ── Scan a single file for correction matches ────────────────────────────────

fn scan_file_for_corrections(
    content: &str,
    sanitized: &[u8],
    corrections: &[(String, String, String)],
    rel_path: &str,
    findings: &mut Vec<TermFinding>,
    internal_findings: &mut Vec<InternalFinding>,
    claimed: &mut BTreeSet<(String, usize)>,
) {
    for (original, correct, term_name) in corrections {
        let orig_bytes = original.as_bytes();
        if orig_bytes.is_empty() {
            continue;
        }
        let mut search_start = 0;
        while search_start < sanitized.len() {
            // Find next occurrence of original in sanitized
            match find_subseq(&sanitized[search_start..], orig_bytes) {
                Some(rel_offset) => {
                    let abs_offset = search_start + rel_offset;
                    let key = (rel_path.to_string(), abs_offset);
                    if claimed.contains(&key) {
                        // Already claimed by a previous term
                        search_start = abs_offset + 1;
                        continue;
                    }
                    claimed.insert(key);

                    let (line, col) = byte_offset_to_line_col(content, abs_offset);
                    let finding = TermFinding {
                        path: rel_path.to_string(),
                        line,
                        column: col,
                        original: original.clone(),
                        correct: correct.clone(),
                        term: term_name.clone(),
                    };
                    findings.push(finding);

                    internal_findings.push(InternalFinding {
                        path: rel_path.to_string(),
                        byte_offset: abs_offset,
                        original_len: orig_bytes.len(),
                        original: original.clone(),
                        correct: correct.clone(),
                    });

                    search_start = abs_offset + 1;
                }
                None => break,
            }
        }
    }
}

/// Find subsequence `needle` in `haystack`, accounting for \0 placeholders.
/// Returns Some(offset) if found, None otherwise.
fn find_subseq(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| {
        // First byte must match exactly (not \0) — prevents matching inside exempt regions
        if w[0] == 0 {
            return false;
        }
        w.iter().zip(needle.iter()).all(|(&h, &n)| h == n || h == 0)
    })
}

// ── US4: FixSpan + apply_fixes (T031) ───────────────────────────────────────

struct FixSpan {
    start: usize,
    end: usize,
    replacement: String,
}

fn apply_fixes(content: &[u8], spans: &[FixSpan]) -> Vec<u8> {
    let mut result = Vec::with_capacity(content.len());
    let mut last_end = 0;
    for span in spans {
        // Copy unchanged bytes before this span
        result.extend_from_slice(&content[last_end..span.start]);
        // Write replacement
        result.extend_from_slice(span.replacement.as_bytes());
        last_end = span.end;
    }
    // Copy remaining bytes
    if last_end < content.len() {
        result.extend_from_slice(&content[last_end..]);
    }
    result
}

// ── US5: learn_correction (T036) ────────────────────────────────────────────

pub fn learn_correction(
    project_root: &Path,
    original: &str,
    correct: &str,
) -> Result<(Term, bool)> {
    if original.trim().is_empty() {
        return Err(MfError::usage("--original cannot be empty", None));
    }
    if correct.trim().is_empty() {
        return Err(MfError::usage("--correct cannot be empty", None));
    }

    let mut index = load_index(project_root)?;
    let idx = find_term_by_correct(&index, correct)?;
    let term_clone = {
        let terms = index.terms.as_mut().ok_or_else(|| {
            MfError::internal("index has no terms array despite successful find_term_by_correct")
        })?;
        let term = &mut terms[idx];

        // Check original doesn't equal term or any alias
        if original == term.term || term.aliases.iter().any(|a| a == original) {
            return Err(MfError::usage(
                format!("'{original}' is already a recognized form for term '{}'", term.term),
                None,
            ));
        }

        // Check idempotent
        let already_exists =
            term.corrections.iter().any(|c| c.original == original && c.correct == correct);
        if already_exists {
            return Ok((term.clone(), false));
        }

        term.corrections
            .push(Correction { original: original.to_string(), correct: correct.to_string() });
        term.clone()
    };

    // Sort terms before save (defensive)
    if let Some(ref mut terms) = index.terms {
        sort_terms_by_name(terms);
    }

    save_index(project_root, &index)?;
    Ok((term_clone, true))
}

// ── US6: fix_term (T040) ────────────────────────────────────────────────────

pub fn fix_term(
    project_root: &Path,
    term_name: &str,
    definition: Option<&str>,
    aliases: &[String],
    tags: &[String],
) -> Result<Term> {
    if definition.is_none() && aliases.is_empty() && tags.is_empty() {
        return Err(MfError::usage(
            "at least one of --definition, --alias, --tag must be provided",
            None,
        ));
    }

    let mut index = load_index(project_root)?;

    // Find term by name (strict case-sensitive) and extract data before mutation
    let term_clone = {
        let terms = index.terms.as_mut().ok_or_else(|| {
            MfError::usage(
                format!("term '{term_name}' not found"),
                Some("use 'mf term list' or 'mf term new'".to_string()),
            )
        })?;

        let pos = terms.iter().position(|t| t.term == term_name).ok_or_else(|| {
            MfError::usage(
                format!("term '{term_name}' not found"),
                Some("use 'mf term list' or 'mf term new'".to_string()),
            )
        })?;

        // Check for duplicate entries (dirty data)
        if terms.iter().filter(|t| t.term == term_name).count() > 1 {
            return Err(MfError::IncompatibleSchema {
                path: project_root.join("mind-index.yaml"),
                found: format!("duplicate term '{term_name}'"),
                expected: vec!["unique terms".to_string()],
            });
        }

        let t = &mut terms[pos];

        // Apply definition
        if let Some(def) = definition {
            t.definition = if def.is_empty() { None } else { Some(def.to_string()) };
        }

        // Append aliases with dedup
        if !aliases.is_empty() {
            let existing: Vec<String> = t.aliases.clone();
            for alias in aliases {
                if !existing.contains(alias) {
                    t.aliases.push(alias.clone());
                }
            }
        }

        // Append tags with dedup
        if !tags.is_empty() {
            let existing: Vec<String> = t.tags.clone();
            for tag in tags {
                if !existing.contains(tag) {
                    t.tags.push(tag.clone());
                }
            }
        }

        t.clone()
    };

    // Sort terms before save (defensive)
    if let Some(ref mut terms) = index.terms {
        sort_terms_by_name(terms);
    }

    save_index(project_root, &index)?;

    Ok(term_clone)
}

// ── Front-matter parsing (T008) ──────────────────────────────────────────────

enum FrontMatterDecision {
    None,
    Skip,
    Present { end_byte_offset: usize },
}

/// Parse front-matter block to detect `mf_term_lint: skip` / `mf-term-lint: skip`.
fn parse_front_matter_skip_flag(content: &str) -> FrontMatterDecision {
    let bytes = content.as_bytes();
    if bytes.len() < 5 {
        return FrontMatterDecision::None;
    }
    if !(bytes[0] == b'-' && bytes[1] == b'-' && bytes[2] == b'-') {
        return FrontMatterDecision::None;
    }
    let after_opener = if bytes[3] == b'\n' {
        4
    } else if bytes.len() > 4 && bytes[3] == b'\r' && bytes[4] == b'\n' {
        5
    } else {
        return FrontMatterDecision::None;
    };

    let closing = find_front_matter_close(bytes, after_opener);
    let end_offset = match closing {
        Some(pos) => pos,
        None => return FrontMatterDecision::None,
    };

    let fm_text = &content[after_opener..end_offset];
    for line in fm_text.lines() {
        let trimmed = line.trim();
        if trimmed == "mf_term_lint: skip" || trimmed == "mf-term-lint: skip" {
            return FrontMatterDecision::Skip;
        }
    }

    let close_end = if end_offset + 4 <= bytes.len() && bytes[end_offset + 3] == b'\r' {
        end_offset + 5
    } else {
        end_offset + 4
    };
    FrontMatterDecision::Present { end_byte_offset: close_end }
}

fn find_front_matter_close(bytes: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    while i + 3 < bytes.len() {
        if bytes[i] == b'-'
            && bytes[i + 1] == b'-'
            && bytes[i + 2] == b'-'
            && (i == start || bytes[i - 1] == b'\n')
        {
            return Some(i);
        }
        i += 1;
    }
    None
}

// ── Strip exempt regions (T009) ──────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScanCursor {
    Body,
    FencedCodeBacktick,
    FencedCodeTilde,
    InlineCode,
    HtmlComment,
    LinkUrl,
    BareUrl,
    BlockExempt,
}

/// Single-pass byte-level state machine that replaces exempt regions with `\0`.
/// Output is the same length as `content.as_bytes()`.
pub(crate) fn strip_exempt_regions(content: &str, fm_end: Option<usize>) -> Vec<u8> {
    let bytes = content.as_bytes();
    let len = bytes.len();
    let mut out = vec![0u8; len];
    let mut state = ScanCursor::Body;

    let start_offset = fm_end.unwrap_or_default();

    let mut i = start_offset;
    while i < len {
        match state {
            ScanCursor::Body => {
                // Check for block-exempt marker
                if i + 3 < len
                    && bytes[i] == b'<'
                    && bytes[i + 1] == b'!'
                    && bytes[i + 2] == b'-'
                    && bytes[i + 3] == b'-'
                {
                    let is_line_start = i == start_offset || bytes[i - 1] == b'\n';
                    if is_line_start {
                        if let Some(comment_end) = find_comment_close(bytes, i + 4) {
                            if content[i + 4..comment_end].trim() == "mf-term-lint:off" {
                                let end = (comment_end + 3).min(len);
                                out[i..end].copy_from_slice(&bytes[i..end]);
                                i = comment_end + 3;
                                state = ScanCursor::BlockExempt;
                                continue;
                            }
                        }
                    }
                    state = ScanCursor::HtmlComment;
                    out[i] = bytes[i];
                    i += 1;
                    continue;
                }

                // Fenced code blocks at line start
                if (bytes[i] == b'`' || bytes[i] == b'~')
                    && is_line_start_pos(bytes, i, start_offset)
                {
                    let fence_len = count_repeated(&bytes[i..], bytes[i]);
                    if fence_len >= 3 {
                        state = match bytes[i] {
                            b'`' => ScanCursor::FencedCodeBacktick,
                            _ => ScanCursor::FencedCodeTilde,
                        };
                        let end = (i + fence_len).min(len);
                        out[i..end].copy_from_slice(&bytes[i..end]);
                        i += fence_len;
                        continue;
                    }
                }

                // Inline code
                if bytes[i] == b'`' {
                    state = ScanCursor::InlineCode;
                    out[i] = bytes[i];
                    i += 1;
                    continue;
                }

                // HTML comment `<!--`
                if i + 3 < len
                    && bytes[i] == b'<'
                    && bytes[i + 1] == b'!'
                    && bytes[i + 2] == b'-'
                    && bytes[i + 3] == b'-'
                {
                    state = ScanCursor::HtmlComment;
                    // opening `<` is zeroed along with the comment content
                    i += 1;
                    continue;
                }

                // Markdown link URL: `](`
                if i + 1 < len && bytes[i] == b']' && bytes[i + 1] == b'(' {
                    state = ScanCursor::LinkUrl;
                    out[i] = bytes[i];
                    i += 1;
                    continue;
                }

                // Bare URL
                if i + 7 < len
                    && bytes[i] == b'h'
                    && bytes[i + 1] == b't'
                    && bytes[i + 2] == b't'
                    && bytes[i + 3] == b'p'
                    && bytes[i + 4] == b':'
                {
                    let colon_pos = i + 4;
                    if colon_pos + 2 < len
                        && bytes[colon_pos + 1] == b'/'
                        && bytes[colon_pos + 2] == b'/'
                    {
                        state = ScanCursor::BareUrl;
                        out[i] = bytes[i];
                        i += 1;
                        continue;
                    }
                }
                // Also check https://
                if i + 8 < len
                    && bytes[i] == b'h'
                    && bytes[i + 1] == b't'
                    && bytes[i + 2] == b't'
                    && bytes[i + 3] == b'p'
                    && bytes[i + 4] == b's'
                    && bytes[i + 5] == b':'
                {
                    let colon_pos = i + 5;
                    if colon_pos + 2 < len
                        && bytes[colon_pos + 1] == b'/'
                        && bytes[colon_pos + 2] == b'/'
                    {
                        state = ScanCursor::BareUrl;
                        out[i] = bytes[i];
                        i += 1;
                        continue;
                    }
                }

                out[i] = bytes[i];
                i += 1;
            }
            ScanCursor::FencedCodeBacktick | ScanCursor::FencedCodeTilde => {
                let fence_char = match state {
                    ScanCursor::FencedCodeBacktick => b'`',
                    _ => b'~',
                };
                if i == start_offset || bytes[i] == b'\n' {
                    let check_start = if bytes[i] == b'\n' { i + 1 } else { i };
                    if check_start < len && bytes[check_start] == fence_char {
                        let fence_len = count_repeated(&bytes[check_start..], fence_char);
                        if fence_len >= 3 {
                            let end = (check_start + fence_len).min(len);
                            out[check_start..end].copy_from_slice(&bytes[check_start..end]);
                            i = check_start + fence_len;
                            state = ScanCursor::Body;
                            continue;
                        }
                    }
                }
                if bytes[i] == b'\r' || bytes[i] == b'\n' {
                    out[i] = bytes[i];
                }
                i += 1;
            }
            ScanCursor::InlineCode => {
                if bytes[i] == b'`' {
                    out[i] = bytes[i];
                    state = ScanCursor::Body;
                    i += 1;
                    continue;
                }
                if bytes[i] == b'\n' {
                    out[i] = bytes[i];
                    state = ScanCursor::Body;
                    i += 1;
                    continue;
                }
                i += 1;
            }
            ScanCursor::HtmlComment => {
                if bytes[i] == b'-' && i + 2 < len && bytes[i + 1] == b'-' && bytes[i + 2] == b'>' {
                    out[i] = bytes[i];
                    out[i + 1] = bytes[i + 1];
                    out[i + 2] = bytes[i + 2];
                    i += 3;
                    state = ScanCursor::Body;
                    continue;
                }
                // Content inside HTML comment is zeroed (not writen to out)
                i += 1;
            }
            ScanCursor::LinkUrl => {
                if bytes[i] == b')' {
                    out[i] = bytes[i];
                    state = ScanCursor::Body;
                    i += 1;
                    continue;
                }
                if bytes[i] == b'\n' {
                    out[i] = bytes[i];
                    state = ScanCursor::Body;
                    i += 1;
                    continue;
                }
                i += 1;
            }
            ScanCursor::BareUrl => {
                if bytes[i] == b' ' || bytes[i] == b'\t' || bytes[i] == b'\n' || bytes[i] == b'\r' {
                    out[i] = bytes[i];
                    state = ScanCursor::Body;
                    i += 1;
                    continue;
                }
                if bytes[i] == b'>' || bytes[i] == b')' {
                    out[i] = bytes[i];
                    state = ScanCursor::Body;
                    i += 1;
                    continue;
                }
                i += 1;
            }
            ScanCursor::BlockExempt => {
                if i + 3 < len
                    && bytes[i] == b'<'
                    && bytes[i + 1] == b'!'
                    && bytes[i + 2] == b'-'
                    && bytes[i + 3] == b'-'
                {
                    let is_line_start = i == start_offset || bytes[i - 1] == b'\n';
                    if is_line_start {
                        if let Some(comment_end) = find_comment_close(bytes, i + 4) {
                            if content[i + 4..comment_end].trim() == "mf-term-lint:on" {
                                let end = (comment_end + 3).min(len);
                                out[i..end].copy_from_slice(&bytes[i..end]);
                                i = comment_end + 3;
                                state = ScanCursor::Body;
                                continue;
                            }
                        }
                    }
                }
                if bytes[i] == b'\n' || bytes[i] == b'\r' {
                    out[i] = bytes[i];
                }
                i += 1;
            }
        }
    }

    out
}

fn find_comment_close(bytes: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    while i + 2 < bytes.len() {
        if bytes[i] == b'-' && bytes[i + 1] == b'-' && bytes[i + 2] == b'>' {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn is_line_start_pos(bytes: &[u8], i: usize, start_offset: usize) -> bool {
    i == start_offset || (i > 0 && bytes[i - 1] == b'\n')
}

fn count_repeated(slice: &[u8], byte: u8) -> usize {
    slice.iter().take_while(|&&b| b == byte).count()
}

// ── Helper: MfError::internal ────────────────────────────────────────────────

impl MfError {
    fn internal(msg: impl Into<String>) -> Self {
        MfError::Internal(anyhow::anyhow!(msg.into()))
    }
}

// ── Tests (unit) ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_front_matter_skip_flag tests ──

    #[test]
    fn fm_none_when_no_front_matter() {
        let content = "Just text\nno front matter\n";
        assert!(matches!(parse_front_matter_skip_flag(content), FrontMatterDecision::None));
    }

    #[test]
    fn fm_present_no_skip() {
        let content = "---\ntitle: test\n---\nbody text";
        match parse_front_matter_skip_flag(content) {
            FrontMatterDecision::Present { end_byte_offset } => {
                assert!(end_byte_offset > 0);
            }
            _ => panic!("expected Present"),
        }
    }

    #[test]
    fn fm_skip_mf_term_lint() {
        let content = "---\nmf_term_lint: skip\n---\nbody text";
        assert!(matches!(parse_front_matter_skip_flag(content), FrontMatterDecision::Skip));
    }

    #[test]
    fn fm_skip_mf_dash_term_lint() {
        let content = "---\nmf-term-lint: skip\n---\nbody text";
        assert!(matches!(parse_front_matter_skip_flag(content), FrontMatterDecision::Skip));
    }

    #[test]
    fn fm_crlf_handling() {
        let content = "---\r\nmf_term_lint: skip\r\n---\r\nbody";
        assert!(matches!(parse_front_matter_skip_flag(content), FrontMatterDecision::Skip));
    }

    #[test]
    fn fm_skip_leading_spaces() {
        let content = "---\n  mf_term_lint: skip\n---\nbody";
        assert!(matches!(parse_front_matter_skip_flag(content), FrontMatterDecision::Skip));
    }

    // ── strip_exempt_regions tests ──

    #[test]
    fn strip_plain_text_preserved() {
        let content = "hello world";
        let result = strip_exempt_regions(content, None);
        assert_eq!(result.len(), content.len());
        assert_eq!(result, content.as_bytes());
    }

    #[test]
    fn strip_fenced_code_block_exempt() {
        let content = "before\n```\ncode mindrepo\n```\nafter";
        let result = strip_exempt_regions(content, None);
        assert_eq!(result.len(), content.len());
        assert_eq!(&result[..7], b"before\n");
        // "code mindrepo" starts at byte 11 (after "before\n```\n")
        let code_start = 11;
        let code_end = code_start + "code mindrepo".len();
        for &b in &result[code_start..code_end] {
            assert_eq!(b, 0, "code block should be zeroed");
        }
        let after_start = content.rfind("after").unwrap();
        assert_eq!(&result[after_start..], b"after");
    }

    #[test]
    fn strip_inline_code_exempt() {
        let content = "text `code mindrepo` more";
        let result = strip_exempt_regions(content, None);
        assert_eq!(result.len(), content.len());
        let start = content.find('`').unwrap();
        let end = content.rfind('`').unwrap();
        for &b in &result[start + 1..end] {
            assert_eq!(b, 0, "inline code should be zeroed");
        }
    }

    #[test]
    fn strip_link_url_exempt() {
        let content = "[text](https://example.com/mindrepo)";
        let result = strip_exempt_regions(content, None);
        assert_eq!(result.len(), content.len());
        let paren_start = content.find('(').unwrap();
        let paren_end = content.find(')').unwrap();
        for &b in &result[paren_start + 1..paren_end] {
            assert_eq!(b, 0, "link URL should be zeroed");
        }
    }

    #[test]
    fn strip_bare_url_exempt() {
        let content = "visit https://example.com/mindrepo for info";
        let result = strip_exempt_regions(content, None);
        assert_eq!(result.len(), content.len());
        // The state machine copies the first 'h', then transitions to BareUrl
        // starting from the next byte. URL starts at 6, first 'h' is preserved.
        let url_content_start = content.find("https://").unwrap() + 1; // skip first 'h'
        let url_end = content[url_content_start..]
            .find(' ')
            .map(|p| url_content_start + p)
            .unwrap_or(content.len());
        for &b in &result[url_content_start..url_end] {
            assert_eq!(b, 0, "bare URL should be zeroed");
        }
    }

    #[test]
    fn strip_front_matter_exempt() {
        let content = "---\ntitle: test\n---\nbody mindrepo";
        let fm_result = parse_front_matter_skip_flag(content);
        let fm_end = match fm_result {
            FrontMatterDecision::Present { end_byte_offset } => Some(end_byte_offset),
            _ => None,
        };
        let result = strip_exempt_regions(content, fm_end);
        assert_eq!(result.len(), content.len());
        if let Some(end) = fm_end {
            for &b in &result[..end] {
                assert_eq!(b, 0, "front matter should be zeroed");
            }
        }
        let body_start = content.find("body").unwrap();
        assert_eq!(&result[body_start..], b"body mindrepo");
    }

    #[test]
    fn strip_block_exempt_markers() {
        let content =
            "before\n<!-- mf-term-lint:off -->\nsecret mindrepo\n<!-- mf-term-lint:on -->\nafter";
        let result = strip_exempt_regions(content, None);
        assert_eq!(result.len(), content.len());
        assert_eq!(&result[..7], b"before\n");
        let after_start = content.rfind("after").unwrap();
        assert_eq!(&result[after_start..], b"after");
        let off_pos = content.find("<!-- mf-term-lint:off -->").unwrap();
        let secret_start = off_pos + "<!-- mf-term-lint:off -->\n".len();
        let on_pos = content.find("<!-- mf-term-lint:on -->").unwrap();
        for &b in &result[secret_start..on_pos] {
            if b != b'\n' {
                assert_eq!(b, 0, "block-exempt region should be zeroed");
            }
        }
    }

    #[test]
    fn strip_output_len_equals_input_len() {
        let cases = vec![
            "plain text",
            "before\n```\ncode\n```\nafter",
            "text `inline` more",
            "a <!-- comment --> b",
            "[link](url) text",
            "visit http://example.com",
            "---\nkey: val\n---\nbody",
            "before\n<!-- mf-term-lint:off -->\nhidden\n<!-- mf-term-lint:on -->\nafter",
        ];
        for content in cases {
            let fm_end = match parse_front_matter_skip_flag(content) {
                FrontMatterDecision::Present { end_byte_offset } => Some(end_byte_offset),
                _ => None,
            };
            let result = strip_exempt_regions(content, fm_end);
            assert_eq!(result.len(), content.len(), "length mismatch for: {content:?}");
        }
    }

    #[test]
    fn fenced_code_prevents_match() {
        let content = "text mindrepo\n```\nmindrepo in code\n```\nmindrepo after";
        let result = strip_exempt_regions(content, None);
        let mid = content.find("```").unwrap();
        let close = content.rfind("```").unwrap();
        let inside_start = mid + 4;
        for &b in &result[inside_start..close] {
            if b != 0 && b != b'\n' {
                panic!("expected zeroed content in code block");
            }
        }
    }

    #[test]
    fn tmux_fence_works() {
        let content = "text\n~~~\nfenced mindrepo\n~~~\nmore";
        let result = strip_exempt_regions(content, None);
        assert_eq!(result.len(), content.len());
        let fence_start = content.find("~~~").unwrap();
        let fence_close = content.rfind("~~~").unwrap();
        let between_start = fence_start + 4;
        for &b in &result[between_start..fence_close] {
            if b != 0 && b != b'\n' {
                panic!("expected zeroed content in tilde-fenced block");
            }
        }
    }

    // ── dedup_preserve_first ──

    #[test]
    fn dedup_removes_duplicates() {
        let items = vec!["a".into(), "b".into(), "a".into(), "c".into()];
        let result = dedup_preserve_first(&items);
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn dedup_empty() {
        assert_eq!(dedup_preserve_first(&[]), Vec::<String>::new());
    }

    // ── byte_offset_to_line_col ──

    #[test]
    fn byte_offset_basic() {
        let content = "hello\nworld";
        let (line, col) = byte_offset_to_line_col(content, 6); // 'w' is at byte 6
        assert_eq!(line, 2);
        assert_eq!(col, 1);
    }

    #[test]
    fn byte_offset_first_line() {
        let content = "hello";
        let (line, col) = byte_offset_to_line_col(content, 0);
        assert_eq!(line, 1);
        assert_eq!(col, 1);
    }

    // ── apply_fixes ──

    #[test]
    fn apply_single_fix() {
        let content = b"hello world";
        let spans = vec![FixSpan { start: 6, end: 11, replacement: "there".to_string() }];
        let result = apply_fixes(content, &spans);
        assert_eq!(result, b"hello there");
    }

    #[test]
    fn apply_multiple_fixes() {
        let content = b"aa bb aa";
        let spans = vec![
            FixSpan { start: 0, end: 2, replacement: "x".to_string() },
            FixSpan { start: 6, end: 8, replacement: "y".to_string() },
        ];
        let result = apply_fixes(content, &spans);
        assert_eq!(result, b"x bb y");
    }

    // ── find_subseq ──

    #[test]
    fn find_subseq_exact() {
        let haystack = b"hello world";
        let needle = b"world";
        assert_eq!(find_subseq(haystack, needle), Some(6));
    }

    #[test]
    fn find_subseq_rejects_all_zeroes() {
        // All-\0 region (exempt) should NOT match — prevents false positives
        let haystack = b"\0\0\0\0\0\0\0\0";
        let needle = b"mindrepo";
        assert_eq!(find_subseq(haystack, needle), None);
    }

    #[test]
    fn find_subseq_allows_zero_in_middle() {
        // \0 mid-match is allowed (spanning across an exempt region)
        let haystack = b"mind\0epo";
        let needle = b"mindrepo";
        assert_eq!(find_subseq(haystack, needle), Some(0));
    }

    #[test]
    fn find_subseq_not_found() {
        let haystack = b"hello world";
        let needle = b"xyz";
        assert_eq!(find_subseq(haystack, needle), None);
    }
}
