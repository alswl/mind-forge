// Source service - implemented in 011-source-core

use std::path::Path;
use std::path::PathBuf;

use chrono::Utc;

use crate::error::{MfError, Result};
use crate::model::index::IndexFile;
use crate::service::util;

use crate::model::source::Source;
use crate::model::source::SourceIndexEntry;
use crate::model::source::SourceIndexReport;
use crate::model::source::SourceKind;
use crate::model::source::SourceRemoveReport;

// ---------------------------------------------------------------------------
// T006: Index I/O

/// Load `mind-index.yaml` from the project root.
/// Returns a default `IndexFile` with empty `sources` when the file is missing.
pub fn load_index(project_root: &Path) -> Result<IndexFile> {
    let path = project_root.join("mind-index.yaml");
    if !path.exists() {
        let mut index = IndexFile::create_default();
        index.sources = Some(Vec::new());
        return Ok(index);
    }
    let content = std::fs::read_to_string(&path).map_err(MfError::Io)?;
    if content.trim().is_empty() {
        let mut index = IndexFile::create_default();
        index.sources = Some(Vec::new());
        return Ok(index);
    }
    let index: IndexFile = serde_yaml::from_str(&content).map_err(|e| MfError::ParseError {
        kind: "yaml".to_string(),
        path: path.clone(),
        detail: e.to_string(),
    })?;
    util::validate_schema_version(&index.schema_version, &path)?;
    Ok(index)
}

/// Save `mind-index.yaml` at the project root using atomic write.
pub fn save_index(index: &IndexFile, project_path: &Path) -> Result<()> {
    let path = project_path.join("mind-index.yaml");
    let content = serde_yaml::to_string(index).map_err(|e| MfError::Internal(e.into()))?;
    util::atomic_write(&path, &content)
}

// ---------------------------------------------------------------------------
// T007: URL validation
// ---------------------------------------------------------------------------

/// Validate that `s` is a basic http(s) URL with a non-empty host segment.
pub(crate) fn validate_url(s: &str) -> Result<()> {
    if !(s.starts_with("http://") || s.starts_with("https://")) {
        return Err(MfError::usage(
            format!("invalid URL '{s}': must start with http:// or https:// and include a host"),
            None as Option<String>,
        ));
    }
    // Check host segment is non-empty
    let after_scheme =
        s.strip_prefix("https://").or_else(|| s.strip_prefix("http://")).unwrap_or("");
    if after_scheme.is_empty() {
        return Err(MfError::usage(
            format!("invalid URL '{s}': must start with http:// or https:// and include a host"),
            None as Option<String>,
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// T008: Derive name from path
// ---------------------------------------------------------------------------

/// Derive a source name from a file path using `file_stem`.
/// Returns a usage error if the stem is empty.
pub(crate) fn derive_name_from_path(p: &Path) -> Result<String> {
    let stem = p.file_stem().and_then(|s| s.to_str()).map(|s| s.to_string()).ok_or_else(|| {
        MfError::usage(
            format!("cannot derive name from path '{}'", p.display()),
            Some("pass --name <STRING>".to_string()),
        )
    })?;
    if stem.is_empty() {
        return Err(MfError::usage(
            format!("cannot derive name from path '{}'", p.display()),
            Some("pass --name <STRING>".to_string()),
        ));
    }
    Ok(stem)
}

// ---------------------------------------------------------------------------
// T009: Infer SourceKind from path
// ---------------------------------------------------------------------------

pub(crate) fn infer_kind_from_path(p: &Path) -> crate::model::source::SourceKind {
    let ext = p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some("pdf") => crate::model::source::SourceKind::Pdf,
        _ => crate::model::source::SourceKind::File,
    }
}

// ---------------------------------------------------------------------------
// T010: classify input — split PATH from URL
// ---------------------------------------------------------------------------

/// Internal enum for classifying `mf source add` input.
pub(crate) enum InputForm {
    Path,
    Url,
}

// Classify an input string as either a file path or a URL.
pub(crate) fn classify_input(input: &str) -> InputForm {
    if input.starts_with("http://") || input.starts_with("https://") {
        InputForm::Url
    } else {
        InputForm::Path
    }
}

// ---------------------------------------------------------------------------
// T018: AddArgs, AddOutcome, AddMode, and add() — Path branch
// ---------------------------------------------------------------------------

/// The mode used when adding a source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddMode {
    Copy,
    Link,
    Url,
}

/// Outcome returned by the `add()` function.
pub struct AddOutcome {
    pub source: Source,
    pub mode: AddMode,
    pub replaced: bool,
}

/// Parameters for `add()`.
///
/// `kind`: `None` means auto-infer from path/URL; `Some(k)` sets the kind explicitly.
pub struct AddArgs<'a> {
    pub input: &'a str,
    pub name: Option<&'a str>,
    pub kind: Option<SourceKind>,
    pub link: bool,
    pub force: bool,
}

/// Result of locating a source slot — either new or replacing an existing entry.
enum UpsertSlot<'a> {
    New,
    Replace { idx: usize, prior: &'a Source },
}

/// Look up a source by name and determine whether we should create or replace.
fn locate_slot<'a>(
    sources: &'a [Source],
    name: &str,
    force: bool,
    conflict_marker: PathBuf,
) -> Result<UpsertSlot<'a>> {
    match sources.iter().position(|s| s.name == name) {
        None => Ok(UpsertSlot::New),
        Some(idx) if force => Ok(UpsertSlot::Replace { idx, prior: &sources[idx] }),
        Some(_) => Err(MfError::file_exists(conflict_marker)),
    }
}

/// Swap a source into the list: remove old at `idx`, push, re-sort.
fn replace_in_sources(sources: &mut Vec<Source>, idx: usize, source: Source) {
    sources.remove(idx);
    sources.push(source);
    sources.sort_by(|a, b| a.name.cmp(&b.name));
}

// ---------------------------------------------------------------------------
// Symlink helper (platform-specific)
// ---------------------------------------------------------------------------

#[cfg(unix)]
fn create_symlink(src: &Path, dst: &Path) -> Result<()> {
    std::os::unix::fs::symlink(src, dst).map_err(MfError::Io)
}

#[cfg(not(unix))]
fn create_symlink(_src: &Path, _dst: &Path) -> Result<()> {
    Err(MfError::usage(
        "symlink is not supported on this platform",
        Some("omit --link to copy the file".to_string()),
    ))
}

/// Add a source — dispatches to Path or URL branch based on input.
pub fn add(project_path: &Path, cwd: &Path, args: &AddArgs) -> Result<AddOutcome> {
    let form = classify_input(args.input);
    match form {
        InputForm::Url => add_url(project_path, args),
        InputForm::Path => add_path(project_path, cwd, args),
    }
}

// ---------------------------------------------------------------------------
// URL branch: register a URL source (index only, no disk I/O)
// ---------------------------------------------------------------------------

fn add_url(project_path: &Path, args: &AddArgs) -> Result<AddOutcome> {
    // 1. Validate URL
    validate_url(args.input)?;

    // 2. --name is required for URL sources
    let name = args.name.map(|s| s.to_string()).ok_or_else(|| {
        MfError::usage(
            "URL sources require an explicit --name",
            Some("pass --name <STRING>".to_string()),
        )
    })?;

    // 3. Resolve kind
    let model_kind = match args.kind.clone() {
        Some(k) => match k {
            SourceKind::Rss | SourceKind::Web => k,
            SourceKind::Pdf => {
                return Err(MfError::usage(
                    "cannot use --type pdf with a URL input",
                    Some("download the file first, then add the local path".to_string()),
                ))
            }
            SourceKind::File => {
                return Err(MfError::usage(
                    "cannot use --type file with a URL input",
                    Some("download the file first, then add the local path".to_string()),
                ))
            }
        },
        None => SourceKind::Web,
    };

    let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    // 4. Load index, upsert by name
    let mut index = load_index(project_path)?;
    let sources = index.sources.get_or_insert_with(Vec::new);
    let slot = locate_slot(sources, &name, args.force, project_path.join("mind-index.yaml"))?;

    let (mode, source, replaced) = match slot {
        UpsertSlot::Replace { idx, prior } => {
            // --force: replace URL entry, preserve added_at and tags
            let source = Source {
                name: name.clone(),
                kind: model_kind,
                url: Some(args.input.to_string()),
                path: None,
                tags: prior.tags.clone(),
                added_at: prior.added_at.clone(),
                updated_at: now,
            };
            replace_in_sources(sources, idx, source.clone());
            (AddMode::Url, source, true)
        }
        UpsertSlot::New => {
            let source = Source {
                name: name.clone(),
                kind: model_kind,
                url: Some(args.input.to_string()),
                path: None,
                tags: vec![],
                added_at: now.clone(),
                updated_at: now,
            };
            sources.push(source.clone());
            sources.sort_by(|a, b| a.name.cmp(&b.name));
            (AddMode::Url, source, false)
        }
    };

    save_index(&index, project_path)?;
    Ok(AddOutcome { source, mode, replaced })
}

// ---------------------------------------------------------------------------
// Path branch: archive a file into `<project>/sources/<kind>/`
// ---------------------------------------------------------------------------

fn add_path(project_path: &Path, cwd: &Path, args: &AddArgs) -> Result<AddOutcome> {
    // 1. Resolve and canonicalize the input path
    let source_path = cwd.join(args.input);
    let source_canonical = source_path.canonicalize().map_err(|e| {
        MfError::usage(
            format!("cannot resolve source path '{}': {e}", args.input),
            None as Option<String>,
        )
    })?;

    // Must be a regular file
    let metadata = std::fs::metadata(&source_canonical).map_err(|e| {
        MfError::usage(
            format!("cannot access '{}': {e}", source_canonical.display()),
            None as Option<String>,
        )
    })?;
    if !metadata.is_file() {
        return Err(MfError::usage(
            format!("source path '{}' must be an existing regular file", args.input),
            None as Option<String>,
        ));
    }

    // 3. Self-reference check: source is inside <project>/sources/
    let sources_dir = project_path.join("sources");
    if util::canonicalize_within(&sources_dir, &source_canonical).is_ok() {
        return Err(MfError::usage(
            "source file is already inside the project's sources/ directory",
            Some("use 'mf source update <NAME>' to modify metadata".to_string()),
        ));
    }

    // 4. Resolve kind — explicit or infer
    let model_kind = match args.kind.clone() {
        Some(k) => match k {
            SourceKind::Pdf | SourceKind::File => k,
            SourceKind::Rss | SourceKind::Web => {
                return Err(MfError::usage(
                    "cannot use --type rss or --type web with a local file input",
                    Some("pass an http(s):// URL".to_string()),
                ))
            }
        },
        None => infer_kind_from_path(&source_canonical),
    };

    // 5. Derive name
    let name = match args.name {
        Some(n) => n.to_string(),
        None => derive_name_from_path(&source_path)?,
    };

    // 6. Compute destination path
    let basename =
        source_canonical.file_name().map(|s| s.to_string_lossy().to_string()).ok_or_else(|| {
            MfError::usage(
                format!("cannot extract filename from '{}'", source_canonical.display()),
                None as Option<String>,
            )
        })?;
    let kind_dir_name = model_kind.as_str();
    let kind_dir = sources_dir.join(kind_dir_name);
    let dest = kind_dir.join(&basename);

    // Ensure dest parent exists
    std::fs::create_dir_all(&kind_dir).map_err(MfError::Io)?;

    // 7. Load index, upsert by name
    let mut index = load_index(project_path)?;
    let sources = index.sources.get_or_insert_with(Vec::new);
    let slot = locate_slot(sources, &name, args.force, dest.clone())?;

    let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    // File write helper (shared by replace and new branches)
    let write_file = || -> Result<()> {
        if dest.exists() {
            std::fs::remove_file(&dest).map_err(MfError::Io)?;
        }
        if args.link {
            create_symlink(&source_canonical, &dest)
        } else {
            std::fs::copy(&source_canonical, &dest)
                .map_err(|e| {
                    MfError::usage(
                        format!("cannot copy to '{}': {e}", dest.display()),
                        None as Option<String>,
                    )
                })
                .map(|_| ())
        }
    };

    let (mode, source, replaced) = match slot {
        UpsertSlot::Replace { idx, prior } => {
            let rel_path = util::rel_posix_path(project_path, &dest)?;
            let old_path = prior.path.clone();

            // Write the new file
            write_file()?;

            // Clean up old file if path differs
            if let Some(ref old) = old_path {
                if *old != rel_path {
                    let _ = std::fs::remove_file(project_path.join(old));
                }
            }

            let source = Source {
                name: name.clone(),
                kind: model_kind,
                url: None,
                path: Some(rel_path),
                tags: prior.tags.clone(),
                added_at: prior.added_at.clone(),
                updated_at: now,
            };
            let mode = if args.link { AddMode::Link } else { AddMode::Copy };
            replace_in_sources(sources, idx, source.clone());
            (mode, source, true)
        }
        UpsertSlot::New => {
            let rel_path = util::rel_posix_path(project_path, &dest)?;

            // Write the new file
            match args.link {
                true => create_symlink(&source_canonical, &dest)?,
                false => {
                    std::fs::copy(&source_canonical, &dest).map_err(|e| {
                        MfError::usage(
                            format!("cannot copy to '{}': {e}", dest.display()),
                            None as Option<String>,
                        )
                    })?;
                }
            }

            let source = Source {
                name: name.clone(),
                kind: model_kind,
                url: None,
                path: Some(rel_path.clone()),
                tags: vec![],
                added_at: now.clone(),
                updated_at: now,
            };
            let mode = if args.link { AddMode::Link } else { AddMode::Copy };
            sources.push(source.clone());
            sources.sort_by(|a, b| a.name.cmp(&b.name));
            (mode, source, false)
        }
    };

    // Save index
    save_index(&index, project_path)?;

    Ok(AddOutcome { source, mode, replaced })
}

// ---------------------------------------------------------------------------
// T024: list() — list sources with optional filtering
// ---------------------------------------------------------------------------

/// List sources in a project, with optional name substring filter and type filter.
pub fn list(
    project_path: &Path,
    filter: Option<&str>,
    kind: Option<SourceKind>,
) -> Result<Vec<Source>> {
    let index = load_index(project_path)?;
    let mut sources = index.sources.unwrap_or_default();

    // Apply substring filter (case-insensitive on name only)
    if let Some(f) = filter {
        let lower = f.to_lowercase();
        sources.retain(|s| s.name.to_lowercase().contains(&lower));
    }

    // Apply type filter
    if let Some(k) = kind {
        sources.retain(|s| s.kind == k);
    }

    // Alphabetical sort by name
    sources.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(sources)
}

// ---------------------------------------------------------------------------
// T028: UpdateArgs and update() — modify source metadata by name
// ---------------------------------------------------------------------------

pub struct UpdateArgs<'a> {
    pub name: &'a str,
    pub rename: Option<&'a str>,
    pub url: Option<&'a str>,
}

/// Update a source by name. At least one of rename or url must be provided.
/// Returns usage error if the source is not found or rename collides with an existing entry.
pub fn update(project_path: &Path, args: &UpdateArgs) -> Result<Source> {
    // At least one change flag required
    if args.rename.is_none() && args.url.is_none() {
        return Err(MfError::usage(
            "nothing to update: use --rename or --url",
            Some("pass --rename <NAME> or --url <URL> to modify the source".to_string()),
        ));
    }

    let mut index = load_index(project_path)?;
    let sources = index.sources.as_mut().ok_or_else(|| {
        MfError::usage(
            format!("source '{}' not found", args.name),
            Some("use 'mf source list' to see available sources".to_string()),
        )
    })?;

    // Find the entry by name
    let idx = sources.iter().position(|s| s.name == args.name).ok_or_else(|| {
        MfError::usage(
            format!("source '{}' not found", args.name),
            Some("use 'mf source list' to see available sources".to_string()),
        )
    })?;

    // Check rename collision (against other entries, excluding self)
    if let Some(new_name) = args.rename {
        if new_name != args.name && sources.iter().any(|s| s.name == new_name) {
            return Err(MfError::usage(
                format!("a source named '{new_name}' already exists"),
                Some("use a different --rename value".to_string()),
            ));
        }
    }

    // Validate URL if provided
    if let Some(u) = args.url {
        validate_url(u)?;
    }

    let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let entry = &mut sources[idx];

    // Apply changes
    if let Some(new_name) = args.rename {
        entry.name = new_name.to_string();
    }
    if let Some(new_url) = args.url {
        entry.url = Some(new_url.to_string());
    }
    entry.updated_at = now.clone();

    // Reconstruct for return
    let updated = entry.clone();

    // Re-sort (rename may affect order)
    sources.sort_by(|a, b| a.name.cmp(&b.name));

    save_index(&index, project_path)?;
    Ok(updated)
}

// ---------------------------------------------------------------------------
// T032: Scan helpers for reconcile (US5)
// ---------------------------------------------------------------------------

/// Shared set of filenames to skip during directory scans.
fn is_skipped_filename(name: &str) -> bool {
    matches!(name, ".DS_Store" | ".gitkeep" | "Thumbs.db")
}

/// Shallow-scan a single-tier source directory (e.g. `sources/pdf/`).
/// Skips hidden files, skipped filenames, and subdirectories.
fn scan_shallow_dir(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut entries = Vec::new();
    if !dir.exists() {
        return Ok(entries);
    }
    let read_dir = std::fs::read_dir(dir).map_err(MfError::Io)?;
    for entry in read_dir {
        let entry = entry.map_err(MfError::Io)?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with('.') || is_skipped_filename(&name_str) {
            continue;
        }
        let ft = entry.file_type().map_err(MfError::Io)?;
        if ft.is_dir() {
            // Skip subdirectories in shallow scan
            continue;
        }
        if ft.is_file() || ft.is_symlink() {
            entries.push(entry.path());
        }
    }
    entries.sort();
    Ok(entries)
}

/// Recursively scan a directory tree (e.g. `sources/file/`).
/// Follows symlinks, skips hidden files and skipped filenames.
fn scan_recursive_dir(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut entries = Vec::new();
    if !dir.exists() {
        return Ok(entries);
    }
    let walker = walkdir::WalkDir::new(dir)
        .follow_links(true)
        .sort_by(|a, b| a.file_name().cmp(b.file_name()));
    for result in walker {
        let entry = result.map_err(|e| MfError::Io(std::io::Error::other(e.to_string())))?;
        let name = entry.file_name().to_string_lossy();
        if name.starts_with('.') || is_skipped_filename(&name) {
            continue;
        }
        if entry.depth() == 0 {
            continue;
        }
        let ft = entry.file_type();
        if ft.is_dir() {
            continue;
        }
        if ft.is_file() || ft.is_symlink() {
            entries.push(entry.path().to_path_buf());
        }
    }
    entries.sort();
    Ok(entries)
}

/// Scan disk files under `sources/pdf/` (shallow) and `sources/file/` (recursive).
/// Returns relative paths (POSIX separators) rooted at the project root.
fn scan_disk_sources(project_path: &Path) -> Result<Vec<PathBuf>> {
    let sources_dir = project_path.join("sources");
    let mut files = Vec::new();

    let pdf_dir = sources_dir.join("pdf");
    for abs_path in scan_shallow_dir(&pdf_dir)? {
        let portable = util::rel_posix_path(project_path, &abs_path)?;
        files.push(PathBuf::from(portable));
    }

    let file_dir = sources_dir.join("file");
    for abs_path in scan_recursive_dir(&file_dir)? {
        let portable = util::rel_posix_path(project_path, &abs_path)?;
        files.push(PathBuf::from(portable));
    }

    files.sort();
    Ok(files)
}

// ---------------------------------------------------------------------------
// T033: reconcile() — reconcile disk files with index (US5)
// ---------------------------------------------------------------------------

/// Reconcile disk files with the index. Returns a report of added/removed/kept.
pub fn reconcile(project_path: &Path, dry_run: bool) -> Result<SourceIndexReport> {
    let sources_dir = project_path.join("sources");
    if !sources_dir.exists() {
        return Err(MfError::usage(
            format!("project has no sources/ directory at '{}'", sources_dir.display()),
            Some("use 'mf project lint --fix' to create missing directories".to_string()),
        ));
    }

    let mut index = load_index(project_path)?;
    let index_sources = index.sources.unwrap_or_default();

    // Separate URL-type sources (always kept) from file-type sources
    let mut url_kept: Vec<Source> = Vec::new();
    let mut indexed_files: Vec<(String, String, SourceKind)> = Vec::new(); // (name, path, kind)
    for s in &index_sources {
        match s.kind {
            SourceKind::Rss | SourceKind::Web => {
                url_kept.push(s.clone());
            }
            SourceKind::Pdf | SourceKind::File => {
                let p = s.path.as_ref().ok_or_else(|| {
                    MfError::Internal(anyhow::anyhow!(
                        "file-type source '{}' has no path field",
                        s.name
                    ))
                })?;
                indexed_files.push((s.name.clone(), p.clone(), s.kind.clone()));
            }
        }
    }

    // Scan disk
    let disk_files = scan_disk_sources(project_path)?;
    let disk_paths: std::collections::BTreeSet<String> =
        disk_files.iter().map(|p| p.to_string_lossy().to_string()).collect();

    // Compute added: on disk but not in index
    let mut added: Vec<SourceIndexEntry> = Vec::new();
    let mut kept_file_paths: Vec<String> = Vec::new();
    for disk_path in &disk_paths {
        let in_index = indexed_files.iter().any(|(_, ip, _)| ip == disk_path);
        if in_index {
            kept_file_paths.push(disk_path.clone());
        } else {
            // New file — derive name from path
            let p = Path::new(disk_path);
            let name = match p.file_stem().and_then(|s| s.to_str()) {
                Some(stem) => stem.to_string(),
                None => continue,
            };
            let kind = infer_kind_from_path(p);
            added.push(SourceIndexEntry { name: name.clone(), kind, path: disk_path.clone() });
        }
    }

    // Compute removed: in index but not on disk (only pdf/file types)
    let mut removed: Vec<SourceIndexEntry> = Vec::new();
    for (name, ip, kind) in &indexed_files {
        if !disk_paths.contains(ip) {
            removed.push(SourceIndexEntry {
                name: name.clone(),
                kind: kind.clone(),
                path: ip.clone(),
            });
        }
    }

    let kept_count = (url_kept.len() as u64) + kept_file_paths.len() as u64;
    let dry_run_value = dry_run;

    if !dry_run {
        // Build new sources vec: kept URL sources + kept file sources + added
        let mut new_sources: Vec<Source> = url_kept.clone();

        // Add kept indexed files
        let by_name: std::collections::HashMap<&str, &Source> =
            index_sources.iter().map(|s| (s.name.as_str(), s)).collect();
        for (name, ip, _kind) in &indexed_files {
            if kept_file_paths.contains(ip) {
                if let Some(orig) = by_name.get(name.as_str()) {
                    new_sources.push((*orig).clone());
                }
            }
        }

        // Add new entries for added files
        let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        for entry in &added {
            new_sources.push(Source {
                name: entry.name.clone(),
                kind: entry.kind.clone(),
                url: None,
                path: Some(entry.path.clone()),
                tags: vec![],
                added_at: now.clone(),
                updated_at: now.clone(),
            });
        }

        new_sources.sort_by(|a, b| a.name.cmp(&b.name));

        index.sources = Some(new_sources);
        save_index(&index, project_path)?;
    }

    // Sort added/removed by name for deterministic output
    added.sort_by(|a, b| a.name.cmp(&b.name));
    removed.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(SourceIndexReport { added, removed, kept_count, dry_run: dry_run_value })
}

// ---------------------------------------------------------------------------
// T037: remove() — remove a source by name, optionally deleting the file
// ---------------------------------------------------------------------------

/// Remove a source by name. If the source is a pdf/file type and `keep_file` is false,
/// the archive file is also deleted. Returns a report with the removed source and
/// whether the file was deleted.
pub fn remove(project_path: &Path, name: &str, keep_file: bool) -> Result<SourceRemoveReport> {
    let mut index = load_index(project_path)?;
    let sources = index.sources.as_mut().ok_or_else(|| {
        MfError::usage(
            format!("source '{name}' not found"),
            Some("use 'mf source list' to see available sources".to_string()),
        )
    })?;

    let idx = sources.iter().position(|s| s.name == name).ok_or_else(|| {
        MfError::usage(
            format!("source '{name}' not found"),
            Some("use 'mf source list' to see available sources".to_string()),
        )
    })?;

    let entry = sources[idx].clone();
    let mut file_deleted = false;

    // Delete file if it's a file-type source and not --keep-file
    if !keep_file && matches!(entry.kind, SourceKind::Pdf | SourceKind::File) {
        if let Some(ref rel_path) = entry.path {
            let abs_path = project_path.join(rel_path);
            match std::fs::remove_file(&abs_path) {
                Ok(_) => file_deleted = true,
                Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => {
                    // File already missing — proceed
                    file_deleted = false;
                }
                Err(e) => {
                    return Err(MfError::Io(e));
                }
            }
        }
    }

    // Remove the entry from the index
    sources.remove(idx);
    save_index(&index, project_path)?;

    Ok(SourceRemoveReport { source: entry, file_deleted })
}

// ---------------------------------------------------------------------------
// T041: clean() — remove index entries whose files are missing (US7)
// ---------------------------------------------------------------------------

/// Clean dirty entries from the index: removes pdf/file sources whose archive
/// files no longer exist on disk. URL sources are always kept. Does NOT discover
/// new files on disk (unlike reconcile).
pub fn clean(project_path: &Path, dry_run: bool) -> Result<SourceIndexReport> {
    let sources_dir = project_path.join("sources");
    if !sources_dir.exists() {
        return Err(MfError::usage(
            format!("project has no sources/ directory at '{}'", sources_dir.display()),
            Some("use 'mf project lint --fix' to create missing directories".to_string()),
        ));
    }

    let mut index = load_index(project_path)?;
    let index_sources = index.sources.unwrap_or_default();

    let mut kept: Vec<Source> = Vec::new();
    let mut removed: Vec<SourceIndexEntry> = Vec::new();

    for s in index_sources {
        match s.kind {
            SourceKind::Rss | SourceKind::Web => {
                // URL sources are always kept
                kept.push(s);
            }
            SourceKind::Pdf | SourceKind::File => {
                let path = s.path.as_ref().ok_or_else(|| {
                    MfError::Internal(anyhow::anyhow!(
                        "file-type source '{}' has no path field",
                        s.name
                    ))
                })?;
                let exists = project_path.join(path).exists();
                if exists {
                    kept.push(s);
                } else {
                    removed.push(SourceIndexEntry {
                        name: s.name.clone(),
                        kind: s.kind.clone(),
                        path: path.clone(),
                    });
                }
            }
        }
    }

    let kept_count = kept.len() as u64;
    let dry_run_value = dry_run;

    if !dry_run && !removed.is_empty() {
        kept.sort_by(|a, b| a.name.cmp(&b.name));
        index.sources = Some(kept);
        save_index(&index, project_path)?;
    }

    // Sort removed by name for deterministic output
    removed.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(SourceIndexReport { added: vec![], removed, kept_count, dry_run: dry_run_value })
}
