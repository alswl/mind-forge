//! Repository-format `minds-terms.yaml` support.
//!
//! Detects whether a terms file uses the schema-version layout or the
//! flat-dictionary (repository) layout. Provides load, save, and
//! surgical-edit operations for repository-format files that preserve
//! trailing comments and blank lines above unchanged entries.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::defaults;
use crate::error::{MfError, Result};
use crate::model::term::{Correction, Term};
use crate::service::util::{atomic_write, validate_schema_version};

// ── Format enum ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TermsFileFormat {
    SchemaVersion,
    Repository,
}

// ── Schema-version file model (mirrors global.rs) ───────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GlobalTermsFile {
    #[serde(alias = "schema")]
    schema_version: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    terms: Vec<Term>,
}

pub(super) const GLOBAL_TERMS_FILENAME: &str = "minds-terms.yaml";

pub(super) fn path_for(repo_root: &Path) -> std::path::PathBuf {
    repo_root.join(GLOBAL_TERMS_FILENAME)
}

// ── Format detection ────────────────────────────────────────────────────────

/// Detect whether `content` is schema-version or repository format.
///
/// Rules (FR-001):
/// 1. Empty / null → Repository.
/// 2. Mapping with `terms` key → SchemaVersion.
/// 3. Mapping with optional `schema_version` (or `schema`) plus entries
///    where every non-schema value is itself a mapping containing a
///    `misrecognitions: [String]` → Repository.
/// 4. Anything else → ParseError.
pub fn detect_format(content: &str, path: &Path) -> Result<TermsFileFormat> {
    if content.trim().is_empty() {
        return Ok(TermsFileFormat::Repository);
    }

    // Pre-scan for duplicate top-level keys before serde_yaml parses
    // (serde_yaml silently drops the first duplicate, or rejects the
    // document entirely on duplicate keys). We want the better error.
    assert_unique_top_level_keys(content, path)?;

    let value: serde_yaml::Value = serde_yaml::from_str(content).map_err(|e| MfError::ParseError {
        kind: "yaml".to_string(),
        path: path.to_path_buf(),
        detail: e.to_string(),
    })?;

    match value {
        serde_yaml::Value::Null => Ok(TermsFileFormat::Repository),
        serde_yaml::Value::Mapping(ref map) => {
            if map.contains_key("terms") {
                return Ok(TermsFileFormat::SchemaVersion);
            }
            validate_optional_repository_schema_version(map, path)?;
            for (key, val) in map.iter() {
                let key_str = key.as_str().ok_or_else(|| MfError::ParseError {
                    kind: "yaml".to_string(),
                    path: path.to_path_buf(),
                    detail: "top-level key must be a string in repository-format minds-terms.yaml".to_string(),
                })?;
                if is_schema_key(key_str) {
                    continue;
                }
                match val {
                    serde_yaml::Value::Mapping(inner) => {
                        match inner.get("misrecognitions") {
                            Some(serde_yaml::Value::Sequence(seq)) => {
                                if !seq.iter().all(|v| v.is_string()) {
                                    return Err(shape_error(path, key_str));
                                }
                            }
                            _ => return Err(shape_error(path, key_str)),
                        }
                    }
                    _ => return Err(shape_error(path, key_str)),
                }
            }
            Ok(TermsFileFormat::Repository)
        }
        _ => Err(MfError::ParseError {
            kind: "yaml".to_string(),
            path: path.to_path_buf(),
            detail: "unsupported file shape: top-level must be a mapping; supported shapes: (a) schema_version: '1' with terms: [...] or (b) flat dictionary <term>: { misrecognitions: [string, ...] }".to_string(),
        }),
    }
}

fn is_schema_key(key: &str) -> bool {
    key == "schema_version" || key == "schema"
}

fn validate_optional_repository_schema_version(map: &serde_yaml::Mapping, path: &std::path::Path) -> Result<()> {
    for key in ["schema_version", "schema"] {
        let Some(value) = map.get(key) else {
            continue;
        };
        let Some(version) = value.as_str() else {
            return Err(MfError::ParseError {
                kind: "yaml".to_string(),
                path: path.to_path_buf(),
                detail: format!("{key} must be a string"),
            });
        };
        validate_schema_version(version, path)?;
    }
    Ok(())
}

fn shape_error(path: &Path, offending_key: &str) -> MfError {
    MfError::ParseError {
        kind: "yaml".to_string(),
        path: path.to_path_buf(),
        detail: format!(
            "unsupported file shape: key '{offending_key}' is not a mapping with misrecognitions: [string, ...]; \
             supported shapes: (a) schema_version: '1' with terms: [...] or \
             (b) flat dictionary <term>: {{ misrecognitions: [string, ...] }}"
        ),
    }
}

// ── Duplicate-key pre-scan ──────────────────────────────────────────────────

/// Scan `content` for duplicate top-level keys (FR-019).
///
/// Line-based scan: records line numbers of every top-level YAML key
/// (first non-whitespace, non-comment line segment before `:`).
/// On the first duplicate returns a ParseError naming the duplicated term
/// and both line numbers.
pub fn assert_unique_top_level_keys(content: &str, path: &Path) -> Result<()> {
    let mut seen: std::collections::BTreeMap<&str, Vec<usize>> = std::collections::BTreeMap::new();

    for (i, line) in content.lines().enumerate() {
        let line_num = i + 1;
        let trimmed = line.trim();
        // Skip empty lines, comments, and indented content
        if trimmed.is_empty() || trimmed.starts_with('#') || line.starts_with(' ') || line.starts_with('\t') {
            continue;
        }
        // Skip YAML special characters at start
        let first_char = trimmed.chars().next().unwrap_or(' ');
        if first_char == '-' || first_char == '{' || first_char == '[' || first_char == '|' || first_char == '>' {
            continue;
        }
        if let Some(colon_pos) = line.find(':') {
            let key = line[..colon_pos].trim_end();
            if !key.is_empty() {
                seen.entry(key).or_default().push(line_num);
            }
        }
    }

    for (key, lines) in &seen {
        if lines.len() > 1 {
            return Err(MfError::ParseError {
                kind: "yaml".to_string(),
                path: path.to_path_buf(),
                detail: format!(
                    "duplicate term '{}' at lines {} and {}; resolve duplicates by hand",
                    key, lines[0], lines[1]
                ),
            });
        }
    }

    Ok(())
}

// ── Load ────────────────────────────────────────────────────────────────────

/// Load terms from `<repo_root>/minds-terms.yaml`.
///
/// On missing file → `Ok((vec![], TermsFileFormat::Repository))`.
/// Schema-version files delegate to the existing `GlobalTermsFile` path.
/// Repository-format files are projected into `Vec<Term>` per data-model.md.
pub fn load(repo_root: &Path) -> Result<(Vec<Term>, TermsFileFormat)> {
    let path = path_for(repo_root);
    let content = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok((vec![], TermsFileFormat::Repository));
        }
        Err(e) => return Err(MfError::from(e)),
    };

    let format = detect_format(&content, &path)?;

    match format {
        TermsFileFormat::SchemaVersion => load_schema_version(&content, &path),
        TermsFileFormat::Repository => load_repository(&content, &path),
    }
}

fn load_schema_version(content: &str, path: &std::path::Path) -> Result<(Vec<Term>, TermsFileFormat)> {
    let file: GlobalTermsFile = serde_yaml::from_str(content).map_err(|e| MfError::ParseError {
        kind: "yaml".to_string(),
        path: path.to_path_buf(),
        detail: e.to_string(),
    })?;
    validate_schema_version(&file.schema_version, path)?;
    Ok((file.terms, TermsFileFormat::SchemaVersion))
}

fn load_repository(content: &str, path: &std::path::Path) -> Result<(Vec<Term>, TermsFileFormat)> {
    assert_unique_top_level_keys(content, path)?;

    // Parse as Value (not Mapping) so serde_yaml doesn't reject duplicate
    // keys — we already screened them above.
    let value: serde_yaml::Value = serde_yaml::from_str(content).map_err(|e| MfError::ParseError {
        kind: "yaml".to_string(),
        path: path.to_path_buf(),
        detail: e.to_string(),
    })?;

    let map = match value {
        serde_yaml::Value::Null => return Ok((vec![], TermsFileFormat::Repository)),
        serde_yaml::Value::Mapping(m) => m,
        _ => {
            return Ok((vec![], TermsFileFormat::Repository));
        }
    };

    let mut terms = Vec::new();
    for (key, val) in &map {
        let term_name = key
            .as_str()
            .ok_or_else(|| MfError::ParseError {
                kind: "yaml".to_string(),
                path: path.to_path_buf(),
                detail: "top-level key must be a string in repository-format minds-terms.yaml".to_string(),
            })?
            .to_string();
        if is_schema_key(&term_name) {
            continue;
        }
        let corrections = if let serde_yaml::Value::Mapping(inner) = val {
            if let Some(serde_yaml::Value::Sequence(seq)) = inner.get("misrecognitions") {
                seq.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| Correction { original: s.to_string(), correct: term_name.clone() })
                    .collect()
            } else {
                vec![]
            }
        } else {
            vec![]
        };
        terms.push(Term { term: term_name, definition: None, aliases: vec![], tags: vec![], corrections });
    }

    Ok((terms, TermsFileFormat::Repository))
}

// ── Save ────────────────────────────────────────────────────────────────────

/// Save terms back to `<repo_root>/minds-terms.yaml`.
///
/// `on_disk_content` is the raw file text the caller already loaded;
/// repository-format writes use it for surgical edits. Schema-version
/// writes ignore it and serialize the full `GlobalTermsFile` fresh.
pub fn save(repo_root: &Path, terms: &[Term], format: TermsFileFormat, on_disk_content: Option<&str>) -> Result<()> {
    let path = path_for(repo_root);
    match format {
        TermsFileFormat::SchemaVersion => {
            let file = GlobalTermsFile { schema_version: defaults::SCHEMA_VERSION.to_string(), terms: terms.to_vec() };
            let yaml = serde_yaml::to_string(&file).map_err(|e| MfError::Internal(e.into()))?;
            atomic_write(&path, &yaml)
        }
        TermsFileFormat::Repository => {
            let content =
                on_disk_content.ok_or_else(|| MfError::internal("repository-format save requires on_disk_content"))?;
            atomic_write(&path, content)
        }
    }
}

// ── Surgical edits ──────────────────────────────────────────────────────────

/// Append a new term entry at end-of-file, preserving all existing bytes.
///
/// Returns the new file content. Inserts a leading blank line if the
/// existing content doesn't end with one. Re-parses the result to
/// confirm it still classifies as Repository format.
pub fn append_term_repo_format(on_disk_content: &str, term: &str, misrecognitions: &[String]) -> Result<String> {
    let mut content = on_disk_content.to_string();

    // Ensure trailing newline then blank line for visual separation
    if !content.ends_with('\n') {
        content.push('\n');
    }
    // If the file isn't empty, add a blank line before the new entry
    if !content.trim().is_empty() {
        // Check if we already have a trailing blank line
        if !content.ends_with("\n\n") {
            content.push('\n');
        }
    }

    // Build the YAML block
    content.push_str(term);
    content.push_str(":\n");
    if misrecognitions.is_empty() {
        content.push_str("  misrecognitions: []\n");
    } else {
        content.push_str("  misrecognitions:\n");
        for m in misrecognitions {
            content.push_str(&format!("    - {m}\n"));
        }
    }

    // Defensive: re-parse to confirm valid Repository format
    let path = std::path::Path::new("minds-terms.yaml");
    let fmt = detect_format(&content, path)?;
    if fmt != TermsFileFormat::Repository {
        return Err(MfError::internal("append produced content that no longer classifies as Repository format"));
    }

    Ok(content)
}

/// Result of appending a misrecognition to a repo-format term entry.
pub struct AppendResult {
    pub content: String,
    pub appended: bool,
}

/// Append a misrecognition to an existing term in a repo-format file.
///
/// Returns `AppendResult { content, appended }`. If the misrecognition already
/// exists for the term, `appended` is `false` and `content` equals the input.
/// If the term is not found, returns a usage error.
pub fn append_misrecognition_repo_format(
    on_disk_content: &str,
    term: &str,
    misrecognition: &str,
) -> Result<AppendResult> {
    // Parse to verify term exists and check for duplicates
    let value: serde_yaml::Value = serde_yaml::from_str(on_disk_content).map_err(|e| MfError::ParseError {
        kind: "yaml".to_string(),
        path: std::path::Path::new("minds-terms.yaml").to_path_buf(),
        detail: e.to_string(),
    })?;

    let map = match &value {
        serde_yaml::Value::Mapping(m) => m,
        _ => {
            return Err(MfError::usage(
                format!("no term registers '{term}' as its main name or alias"),
                Some(format!("register first with 'mf term new {term}'")),
            ));
        }
    };

    let term_entry = map.iter().find(|(k, _)| k.as_str() == Some(term)).map(|(_, v)| v).ok_or_else(|| {
        MfError::usage(
            format!("no term registers '{term}' as its main name or alias"),
            Some(format!("register first with 'mf term new {term}'")),
        )
    })?;

    // Check if misrecognition already exists
    if let serde_yaml::Value::Mapping(inner) = term_entry {
        if let Some(serde_yaml::Value::Sequence(seq)) = inner.get("misrecognitions") {
            if seq.iter().any(|v| v.as_str() == Some(misrecognition)) {
                return Ok(AppendResult { content: on_disk_content.to_string(), appended: false });
            }
        }
    }

    // Line-based surgical edit: locate the term block and insert the misrecognition.
    let lines: Vec<&str> = on_disk_content.lines().collect();

    // A top-level key line: starts at col 0, contains `:`, and the substring
    // before the first `:` (after trimming trailing whitespace) equals the term.
    // This handles both `cafed:` and `cafed:  # comment`, and avoids prefix
    // collisions like `cafe` matching `cafed:`.
    let term_key_line = lines
        .iter()
        .position(|line| match line.find(':') {
            Some(idx) if !line.starts_with(' ') && !line.starts_with('\t') => line[..idx].trim_end() == term,
            _ => false,
        })
        .ok_or_else(|| {
            MfError::usage(
                format!("no term registers '{term}' as its main name or alias"),
                Some(format!("register first with 'mf term new {term}'")),
            )
        })?;

    // Find the misrecognitions line within this term's block
    let mut misrecog_line = None;
    let mut last_misrecog_item_line = None;
    let mut is_inline_empty = false;
    for (i, line) in lines.iter().enumerate().skip(term_key_line + 1) {
        // Stop at next top-level key (non-empty, non-indented, non-comment)
        if !line.is_empty() && !line.starts_with('#') && !line.starts_with(' ') && line.contains(':') {
            break;
        }
        if line.trim_start().starts_with("misrecognitions:") {
            misrecog_line = Some(i);
            if line.trim_end().ends_with("[]") {
                is_inline_empty = true;
            }
        }
        if misrecog_line.is_some() && line.trim_start().starts_with("- ") {
            last_misrecog_item_line = Some(i);
        }
    }

    let misrecog_line = misrecog_line.ok_or_else(|| MfError::internal("term entry missing misrecognitions key"))?;

    let has_trailing_newline = on_disk_content.ends_with('\n');

    let new_content = if is_inline_empty {
        // Replace `misrecognitions: []` with block form including the new item
        let indent = lines[misrecog_line].chars().take_while(|c| *c == ' ').count();
        let mut result: Vec<String> = lines.iter().take(misrecog_line).map(|s| s.to_string()).collect();
        result.push(format!("{:indent$}misrecognitions:", "", indent = indent));
        result.push(format!("{:indent$}  - {misrecognition}", "", indent = indent));
        result.extend(lines.iter().skip(misrecog_line + 1).map(|s| s.to_string()));
        result.join("\n")
    } else if let Some(last_line) = last_misrecog_item_line {
        // Insert after the last misrecognition item
        let mut result: Vec<String> = lines.iter().take(last_line + 1).map(|s| s.to_string()).collect();
        let indent = lines[last_line].chars().take_while(|c| *c == ' ').count();
        result.push(format!("{:indent$}- {misrecognition}", "", indent = indent));
        result.extend(lines.iter().skip(last_line + 1).map(|s| s.to_string()));
        result.join("\n")
    } else {
        // misrecognitions key exists but no items yet (shouldn't normally happen after parse check)
        let indent = lines[misrecog_line].chars().take_while(|c| *c == ' ').count();
        let mut result: Vec<String> = lines.iter().take(misrecog_line + 1).map(|s| s.to_string()).collect();
        result.push(format!("{:indent$}  - {misrecognition}", "", indent = indent));
        result.extend(lines.iter().skip(misrecog_line + 1).map(|s| s.to_string()));
        result.join("\n")
    };

    let new_content =
        if has_trailing_newline && !new_content.ends_with('\n') { format!("{new_content}\n") } else { new_content };

    // Defensive: re-parse to confirm valid Repository format
    let path = std::path::Path::new("minds-terms.yaml");
    let fmt = detect_format(&new_content, path)?;
    if fmt != TermsFileFormat::Repository {
        return Err(MfError::internal(
            "append_misrecognition produced content that no longer classifies as Repository format",
        ));
    }

    Ok(AppendResult { content: new_content, appended: true })
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_path(name: &str) -> std::path::PathBuf {
        let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        base.join("tests/fixtures/term_repo_format").join(name)
    }

    fn read_fixture(name: &str) -> String {
        std::fs::read_to_string(fixture_path(name)).unwrap()
    }

    // ── detect_format ───────────────────────────────────────────────────────

    #[test]
    fn detect_empty_string_is_repository() {
        let path = Path::new("test.yaml");
        assert_eq!(detect_format("", path).unwrap(), TermsFileFormat::Repository);
        assert_eq!(detect_format("   \n  ", path).unwrap(), TermsFileFormat::Repository);
    }

    #[test]
    fn detect_null_is_repository() {
        let content = read_fixture("null.yaml");
        assert_eq!(detect_format(&content, Path::new("null.yaml")).unwrap(), TermsFileFormat::Repository);
    }

    #[test]
    fn detect_schema_version() {
        let content = "schema_version: '1'\nterms: []";
        assert_eq!(detect_format(content, Path::new("test.yaml")).unwrap(), TermsFileFormat::SchemaVersion);
    }

    #[test]
    fn detect_schema_alias() {
        let content = "schema: '1'\nterms: []";
        assert_eq!(detect_format(content, Path::new("test.yaml")).unwrap(), TermsFileFormat::SchemaVersion);
    }

    #[test]
    fn detect_repository_format() {
        let content = read_fixture("simple.yaml");
        assert_eq!(detect_format(&content, Path::new("simple.yaml")).unwrap(), TermsFileFormat::Repository);
    }

    #[test]
    fn detect_schema_tagged_repository_format() {
        let content = "schema_version: '1'\ncafed:\n  misrecognitions:\n    - 凯飞迪\n";
        assert_eq!(detect_format(content, Path::new("tagged.yaml")).unwrap(), TermsFileFormat::Repository);
    }

    #[test]
    fn detect_mixed_is_schema_version() {
        let content = read_fixture("mixed.yaml");
        // terms present → SchemaVersion wins.
        assert_eq!(detect_format(&content, Path::new("mixed.yaml")).unwrap(), TermsFileFormat::SchemaVersion);
    }

    #[test]
    fn detect_malformed_is_error() {
        let content = read_fixture("malformed.yaml");
        let err = detect_format(&content, Path::new("malformed.yaml")).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("unsupported file shape"), "got: {msg}");
    }

    // ── assert_unique_top_level_keys ────────────────────────────────────────

    #[test]
    fn unique_keys_passes() {
        let content = read_fixture("simple.yaml");
        assert_unique_top_level_keys(&content, Path::new("simple.yaml")).unwrap();
    }

    #[test]
    fn duplicate_key_rejected() {
        let content = read_fixture("duplicate.yaml");
        let err = assert_unique_top_level_keys(&content, Path::new("duplicate.yaml")).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("duplicate term"), "got: {msg}");
        assert!(msg.contains("卿祤"), "got: {msg}");
    }

    // ── load ────────────────────────────────────────────────────────────────

    #[test]
    fn load_empty_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let (terms, format) = load(dir.path()).unwrap();
        assert!(terms.is_empty());
        assert_eq!(format, TermsFileFormat::Repository);
    }

    #[test]
    fn load_repository_format() {
        let dir = tempfile::tempdir().unwrap();
        let content = read_fixture("simple.yaml");
        std::fs::write(dir.path().join("minds-terms.yaml"), &content).unwrap();

        let (terms, format) = load(dir.path()).unwrap();
        assert_eq!(format, TermsFileFormat::Repository);
        assert_eq!(terms.len(), 3);
        assert_eq!(terms[0].term, "cafed");
        assert_eq!(terms[0].definition, None);
        assert!(terms[0].aliases.is_empty());
        assert!(terms[0].tags.is_empty());
        assert_eq!(terms[0].corrections.len(), 2);
        assert_eq!(terms[0].corrections[0].original, "凯飞迪");
        assert_eq!(terms[0].corrections[0].correct, "cafed");
        assert_eq!(terms[0].corrections[1].original, "caféd");
    }

    #[test]
    fn load_schema_tagged_repository_format_skips_schema_key() {
        let dir = tempfile::tempdir().unwrap();
        let content = "schema_version: '1'\ncafed:\n  misrecognitions:\n    - 凯飞迪\n";
        std::fs::write(dir.path().join("minds-terms.yaml"), content).unwrap();

        let (terms, format) = load(dir.path()).unwrap();
        assert_eq!(format, TermsFileFormat::Repository);
        assert_eq!(terms.len(), 1);
        assert_eq!(terms[0].term, "cafed");
        assert_eq!(terms[0].corrections[0].original, "凯飞迪");
    }

    #[test]
    fn load_schema_tagged_repository_format_validates_schema_version() {
        let dir = tempfile::tempdir().unwrap();
        let content = "schema_version: '2'\ncafed:\n  misrecognitions:\n    - 凯飞迪\n";
        std::fs::write(dir.path().join("minds-terms.yaml"), content).unwrap();

        let err = load(dir.path()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("incompatible schema"), "got: {msg}");
    }

    #[test]
    fn load_rejects_duplicate() {
        let dir = tempfile::tempdir().unwrap();
        let content = read_fixture("duplicate.yaml");
        std::fs::write(dir.path().join("minds-terms.yaml"), &content).unwrap();

        let err = load(dir.path()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("duplicate term"), "got: {msg}");
    }

    #[test]
    fn load_mixed_is_schema_version() {
        let dir = tempfile::tempdir().unwrap();
        let content = read_fixture("mixed.yaml");
        std::fs::write(dir.path().join("minds-terms.yaml"), &content).unwrap();

        let (terms, format) = load(dir.path()).unwrap();
        assert_eq!(format, TermsFileFormat::SchemaVersion);
        assert!(terms.is_empty()); // terms: [] in the fixture
    }

    #[test]
    fn load_rejects_malformed() {
        let dir = tempfile::tempdir().unwrap();
        let content = read_fixture("malformed.yaml");
        std::fs::write(dir.path().join("minds-terms.yaml"), &content).unwrap();

        let err = load(dir.path()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("unsupported file shape"), "got: {msg}");
    }

    #[test]
    fn load_empty_file_is_repository_zero_terms() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("minds-terms.yaml"), "").unwrap();
        let (terms, format) = load(dir.path()).unwrap();
        assert!(terms.is_empty());
        assert_eq!(format, TermsFileFormat::Repository);
    }

    #[test]
    fn load_null_file_is_repository_zero_terms() {
        let dir = tempfile::tempdir().unwrap();
        let content = read_fixture("null.yaml");
        std::fs::write(dir.path().join("minds-terms.yaml"), &content).unwrap();
        let (terms, format) = load(dir.path()).unwrap();
        assert!(terms.is_empty());
        assert_eq!(format, TermsFileFormat::Repository);
    }
}
