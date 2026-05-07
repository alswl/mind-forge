// Asset service - implemented in 010-asset-core

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use chrono::Utc;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::error::{MfError, Result};
use crate::model::asset::{Asset, AssetIndexEntry, AssetIndexReport, AssetKind, AssetUpdateResult};
use crate::model::index::IndexFile;
use crate::service::util;

// ---------------------------------------------------------------------------
// T006: SHA-256 file hashing
// ---------------------------------------------------------------------------

/// Compute SHA-256 hex digest of a file at `path`.
///
/// Reads with an 8KB buffer to avoid OOM on large files.
/// Follows symlinks (default `File::open` behaviour — research.md R4).
pub fn sha256_file(path: &Path) -> Result<String> {
    let file = fs::File::open(path).map_err(|e| {
        MfError::usage(format!("cannot open '{}': {e}", path.display()), None as Option<String>)
    })?;
    let mut reader = std::io::BufReader::with_capacity(8192, file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let n = reader.read(&mut buffer).map_err(|e| {
            MfError::usage(format!("cannot read '{}': {e}", path.display()), None as Option<String>)
        })?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

// ---------------------------------------------------------------------------
// T007: Extension-based asset kind inference
// ---------------------------------------------------------------------------

const EXTENSION_MAP: &[(&str, AssetKind)] = &[
    ("png", AssetKind::Image),
    ("jpg", AssetKind::Image),
    ("jpeg", AssetKind::Image),
    ("gif", AssetKind::Image),
    ("webp", AssetKind::Image),
    ("svg", AssetKind::Image),
    ("bmp", AssetKind::Image),
    ("mp4", AssetKind::Video),
    ("mov", AssetKind::Video),
    ("webm", AssetKind::Video),
    ("mkv", AssetKind::Video),
    ("avi", AssetKind::Video),
    ("mp3", AssetKind::Audio),
    ("wav", AssetKind::Audio),
    ("flac", AssetKind::Audio),
    ("ogg", AssetKind::Audio),
    ("m4a", AssetKind::Audio),
];

fn infer_kind(extension: Option<&std::ffi::OsStr>) -> AssetKind {
    let ext = extension.and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some(e) => EXTENSION_MAP
            .iter()
            .find(|(k, _)| *k == e)
            .map(|(_, kind)| *kind)
            .unwrap_or(AssetKind::Other),
        None => AssetKind::Other,
    }
}

// ---------------------------------------------------------------------------
// T008: Index file I/O (mirrors service::article pattern)
// ---------------------------------------------------------------------------

/// Load `mind-index.yaml` from the project root.
/// Returns a default `IndexFile` with empty `assets` when the file is missing.
pub fn load_index(project_path: &Path) -> Result<IndexFile> {
    let path = project_path.join("mind-index.yaml");
    if !path.exists() {
        let mut index = IndexFile::create_default();
        index.assets = Some(Vec::new());
        return Ok(index);
    }
    let content = fs::read_to_string(&path).map_err(MfError::Io)?;
    if content.trim().is_empty() {
        let mut index = IndexFile::create_default();
        index.assets = Some(Vec::new());
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
// T011: AddArgs (public parameter struct)
// ---------------------------------------------------------------------------

/// Parameters for `add()`.
pub struct AddArgs {
    pub source: PathBuf,
    pub tags: Vec<String>,
    pub link_mode: bool,
    pub force: bool,
}

// ---------------------------------------------------------------------------
// T012: Symlink helper (platform-specific)
// ---------------------------------------------------------------------------

#[cfg(unix)]
fn create_symlink(src: &Path, dst: &Path) -> Result<()> {
    std::os::unix::fs::symlink(src, dst).map_err(MfError::Io)
}

#[cfg(not(unix))]
fn create_symlink(_src: &Path, _dst: &Path) -> Result<()> {
    Err(MfError::usage(
        "symlink is not supported on this platform",
        Some("use --copy or omit --link to copy the file".to_string()),
    ))
}

// ---------------------------------------------------------------------------
// T011: add() — copy or symlink a file into the project assets/
// ---------------------------------------------------------------------------

/// Add an external file to the project's asset pool.
///
/// Steps:
/// 1. Resolve & canonicalize the source path
/// 2. Validate it's a regular file (not dir, not self-reference)
/// 3. Compute dest = `<project>/assets/<basename>`
/// 4. Check for existing file (--force to overwrite)
/// 5. Copy or symlink the file
/// 6. Compute size + SHA-256 hash
/// 7. Update index (load → merge → sort → save)
pub fn add(project_path: &Path, cwd: &Path, args: &AddArgs) -> Result<Asset> {
    // Resolve source path relative to cwd, then canonicalize
    let source_canonical = cwd.join(&args.source).canonicalize().map_err(|e| {
        MfError::usage(
            format!("cannot resolve source path '{}': {e}", args.source.display()),
            None as Option<String>,
        )
    })?;

    // Must be a regular file
    let metadata = fs::metadata(&source_canonical).map_err(|e| {
        MfError::usage(
            format!("cannot access '{}': {e}", source_canonical.display()),
            None as Option<String>,
        )
    })?;
    if !metadata.is_file() {
        return Err(MfError::usage(
            format!("source path '{}' must be an existing regular file", args.source.display()),
            None as Option<String>,
        ));
    }

    // Ensure assets/ directory exists
    let assets_dir = project_path.join("assets");
    fs::create_dir_all(&assets_dir).map_err(MfError::Io)?;

    // Compute destination: project_root/assets/<basename>
    let basename =
        args.source.file_name().map(|s| s.to_string_lossy().to_string()).ok_or_else(|| {
            MfError::usage(
                format!("cannot extract filename from '{}'", args.source.display()),
                None as Option<String>,
            )
        })?;
    let dest = assets_dir.join(&basename);

    // Self-reference check: source is inside assets/
    let assets_canonical = assets_dir.canonicalize().map_err(|e| {
        MfError::usage(format!("cannot resolve assets directory: {e}"), None as Option<String>)
    })?;
    if source_canonical.starts_with(&assets_canonical) {
        return Err(MfError::usage(
            "source file is already inside the project's assets/ directory",
            Some("use 'mf asset update <PATH>' to refresh metadata".to_string()),
        ));
    }

    // Check if dest already exists
    if dest.exists() && !args.force {
        return Err(MfError::file_exists(dest.canonicalize().unwrap_or(dest)));
    }

    // Copy or symlink
    if args.link_mode {
        // Remove existing if force
        if dest.exists() {
            fs::remove_file(&dest).map_err(MfError::Io)?;
        }
        create_symlink(&source_canonical, &dest)?;
    } else {
        // copy — use atomic write equivalent
        if dest.exists() {
            fs::remove_file(&dest).map_err(MfError::Io)?;
        }
        fs::copy(&source_canonical, &dest).map_err(|e| {
            MfError::usage(
                format!("cannot copy to '{}': {e}", dest.display()),
                None as Option<String>,
            )
        })?;
    }

    // Compute size and hash of the new file (re-read metadata after copy/symlink)
    let dest_metadata = fs::metadata(&dest).map_err(MfError::Io)?;
    let size = dest_metadata.len();
    let hash = sha256_file(&dest)?;

    // Generate path relative to project root
    let rel_path = format!("assets/{basename}");

    // Infer type
    let ext = dest.extension();
    let kind = infer_kind(ext);

    let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let asset = Asset {
        name: basename,
        kind,
        path: rel_path.clone(),
        size,
        hash,
        tags: args.tags.clone(),
        added_at: now,
    };

    // Load index, merge, sort, save
    let mut index = load_index(project_path)?;
    let assets = index.assets.get_or_insert_with(Vec::new);

    if args.force {
        assets.retain(|a| a.path != rel_path);
    }
    assets.push(asset.clone());
    assets.sort_by(|a, b| a.name.cmp(&b.name));

    save_index(&index, project_path)?;

    Ok(asset)
}

// ---------------------------------------------------------------------------
// T017: list() — list assets with optional filtering
// ---------------------------------------------------------------------------

/// List assets in a project, with optional substring filter and type filter.
pub fn list(
    project_path: &Path,
    filter: Option<&str>,
    kind: Option<AssetKind>,
) -> Result<Vec<Asset>> {
    let index = load_index(project_path)?;
    let mut assets = index.assets.unwrap_or_default();

    // Apply substring filter (case-insensitive on name or tags)
    if let Some(f) = filter {
        let lower = f.to_lowercase();
        assets.retain(|a| {
            a.name.to_lowercase().contains(&lower)
                || a.tags.iter().any(|t| t.to_lowercase().contains(&lower))
        });
    }

    // Apply type filter
    if let Some(k) = kind {
        assets.retain(|a| a.kind == k);
    }

    // Alphabetical sort by name
    assets.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(assets)
}

// ---------------------------------------------------------------------------
// T022: resolve_asset_path — three-step path resolution
// ---------------------------------------------------------------------------

/// Resolve a user-provided path to an asset path within `<project>/assets/`.
///
/// Strategy (research.md R6):
/// 1. project_root.join(input)
/// 2. project_root.join("assets").join(input)
/// 3. cwd.join(input)
///    Each candidate is canonicalized and checked to be within `<project>/assets/`.
fn resolve_asset_path(project_root: &Path, cwd: &Path, input: &Path) -> Result<PathBuf> {
    let assets_dir = project_root.join("assets");
    let candidates = [project_root.join(input), assets_dir.join(input), cwd.join(input)];
    for candidate in &candidates {
        if let Ok(canonical) = util::canonicalize_within(&assets_dir, candidate) {
            return Ok(canonical);
        }
    }
    Err(MfError::usage(
        format!("could not resolve '{}' within assets/", input.display()),
        Some("run 'mf asset list'".to_string()),
    ))
}

// ---------------------------------------------------------------------------
// T023: update_one — refresh a single asset's size and hash
// ---------------------------------------------------------------------------

/// Update the size and hash for a single asset identified by `input`.
pub fn update_one(project_path: &Path, cwd: &Path, input: &Path) -> Result<AssetUpdateResult> {
    let resolved = resolve_asset_path(project_path, cwd, input)?;

    // Convert resolved path to project-relative POSIX path
    let project_canonical = project_path.canonicalize().map_err(MfError::Io)?;
    let rel_path = resolved
        .strip_prefix(&project_canonical)
        .map_err(|_| {
            MfError::usage(
                "resolved path is outside the project root".to_string(),
                Some("run 'mf asset list'".to_string()),
            )
        })?
        .to_string_lossy()
        .replace('\\', "/");

    // Load index and find entry
    let mut index = load_index(project_path)?;
    let entry_idx = index
        .assets
        .as_ref()
        .and_then(|assets| assets.iter().position(|a| a.path == rel_path))
        .ok_or_else(|| {
            MfError::usage(
                format!("no index entry for '{rel_path}'"),
                Some("run 'mf asset list'".to_string()),
            )
        })?;

    let entry = &index.assets.as_ref().unwrap()[entry_idx];
    let old_size = entry.size;
    let old_hash = entry.hash.clone();

    // Check disk file exists (including symlink targets)
    if !resolved.exists() {
        return Err(MfError::usage(
            format!("file not found on disk: {rel_path}"),
            Some("run 'mf asset index' to reconcile".to_string()),
        ));
    }

    let metadata = fs::metadata(&resolved).map_err(MfError::Io)?;
    let new_size = metadata.len();
    let new_hash = sha256_file(&resolved)?;
    let changed = new_size != old_size || new_hash != old_hash;

    if changed {
        if let Some(assets) = &mut index.assets {
            if let Some(entry) = assets.get_mut(entry_idx) {
                entry.size = new_size;
                entry.hash = new_hash.clone();
            }
        }
        save_index(&index, project_path)?;
    }

    Ok(AssetUpdateResult {
        path: rel_path,
        changed,
        old_size,
        new_size,
        old_hash,
        new_hash,
        error: None,
    })
}

// ---------------------------------------------------------------------------
// T024: update_all — refresh all assets
// ---------------------------------------------------------------------------

/// Update size and hash for all registered assets.
/// Missing files are recorded with `error = "missing_file"` but do not halt.
pub fn update_all(project_path: &Path) -> Result<Vec<AssetUpdateResult>> {
    let mut index = load_index(project_path)?;
    let assets = index.assets.get_or_insert_with(Vec::new);
    let mut results = Vec::new();
    let mut any_changed = false;

    for entry in assets.iter_mut() {
        let full_path = project_path.join(&entry.path);
        let old_size = entry.size;
        let old_hash = entry.hash.clone();

        if !full_path.exists() {
            results.push(AssetUpdateResult {
                path: entry.path.clone(),
                changed: false,
                old_size,
                new_size: old_size,
                old_hash: old_hash.clone(),
                new_hash: old_hash,
                error: Some("missing_file".to_string()),
            });
            continue;
        }

        let metadata = match fs::metadata(&full_path) {
            Ok(m) => m,
            Err(_) => {
                results.push(AssetUpdateResult {
                    path: entry.path.clone(),
                    changed: false,
                    old_size,
                    new_size: old_size,
                    old_hash: old_hash.clone(),
                    new_hash: old_hash,
                    error: Some("missing_file".to_string()),
                });
                continue;
            }
        };

        let new_size = metadata.len();
        let new_hash = match sha256_file(&full_path) {
            Ok(h) => h,
            Err(_) => {
                results.push(AssetUpdateResult {
                    path: entry.path.clone(),
                    changed: false,
                    old_size,
                    new_size: old_size,
                    old_hash: old_hash.clone(),
                    new_hash: old_hash,
                    error: Some("missing_file".to_string()),
                });
                continue;
            }
        };

        let changed = new_size != old_size || new_hash != old_hash;
        if changed {
            entry.size = new_size;
            entry.hash.clone_from(&new_hash);
            any_changed = true;
        }

        results.push(AssetUpdateResult {
            path: entry.path.clone(),
            changed,
            old_size,
            new_size,
            old_hash,
            new_hash,
            error: None,
        });
    }

    // Sort results by path for deterministic output
    results.sort_by(|a, b| a.path.cmp(&b.path));

    if any_changed {
        save_index(&index, project_path)?;
    }

    Ok(results)
}

// ---------------------------------------------------------------------------
// T029: scan_assets_dir — recursive scan of assets/ directory
// ---------------------------------------------------------------------------

/// Recursively scan `<project>/assets/` for regular files.
///
/// Skips:
/// - Hidden files (basename starts with `.`)
/// - Hard-coded `Thumbs.db`
/// - Broken symlinks
fn scan_assets_dir(assets_dir: &Path) -> Result<Vec<PathBuf>> {
    if !assets_dir.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    for entry in WalkDir::new(assets_dir).follow_links(true).into_iter() {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        // Skip hidden files
        if let Some(name) = entry.file_name().to_str() {
            if name.starts_with('.') || name == "Thumbs.db" {
                continue;
            }
        }

        // Only collect regular files
        if entry.file_type().is_file() {
            files.push(entry.path().to_path_buf());
        }
    }

    Ok(files)
}

// ---------------------------------------------------------------------------
// T030: reconcile — reconcile assets dir with index
// ---------------------------------------------------------------------------

/// Reconcile the filesystem assets directory with the index.
///
/// Returns an `AssetIndexReport` with added, removed, kept, and optionally refreshed items.
/// When `dry_run` is true, the index is not written.
/// When `refresh_metadata` is true, kept entries get size/hash refreshed.
pub fn reconcile(
    project_path: &Path,
    dry_run: bool,
    refresh_metadata: bool,
) -> Result<AssetIndexReport> {
    let assets_dir = project_path.join("assets");
    if !assets_dir.exists() {
        return Err(MfError::usage(
            format!("assets directory not found at '{}'", assets_dir.display()),
            Some("run 'mf project lint --fix' to create missing directories".to_string()),
        ));
    }

    let mut index = load_index(project_path)?;
    let existing_assets = index.assets.clone().unwrap_or_default();

    // Scan disk files
    let disk_files = scan_assets_dir(&assets_dir)?;
    let project_canonical = project_path.canonicalize().map_err(MfError::Io)?;

    // Build sets: indexed paths vs disk paths
    let indexed_paths: std::collections::BTreeSet<String> =
        existing_assets.iter().map(|a| a.path.clone()).collect();
    let disk_paths: std::collections::BTreeSet<String> = disk_files
        .iter()
        .filter_map(|f| {
            f.strip_prefix(&project_canonical).ok().map(|p| p.to_string_lossy().replace('\\', "/"))
        })
        .collect();

    // Added: on disk but not in index
    let mut added: Vec<_> = disk_paths
        .difference(&indexed_paths)
        .map(|p| {
            let full_path = project_path.join(p);
            let name =
                full_path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let size = fs::metadata(&full_path).map(|m| m.len()).unwrap_or(0);
            let hash = sha256_file(&full_path).unwrap_or_default();
            let ext = full_path.extension();
            let kind = infer_kind(ext);
            let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

            // Also add to index
            let asset = Asset {
                name: name.clone(),
                kind,
                path: p.clone(),
                size,
                hash,
                tags: Vec::new(),
                added_at: now,
            };
            index.assets.get_or_insert_with(Vec::new).push(asset);

            AssetIndexEntry { name, path: p.clone() }
        })
        .collect();
    added.sort_by(|a, b| a.name.cmp(&b.name));

    // Removed: in index but not on disk
    let mut removed: Vec<_> = indexed_paths
        .difference(&disk_paths)
        .map(|p| {
            // Find name from existing assets
            let name = existing_assets
                .iter()
                .find(|a| a.path == *p)
                .map(|a| a.name.clone())
                .unwrap_or_else(|| {
                    std::path::Path::new(p)
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default()
                });
            AssetIndexEntry { name, path: p.clone() }
        })
        .collect();
    removed.sort_by(|a, b| a.name.cmp(&b.name));

    // Remove from index
    let remove_paths: std::collections::HashSet<String> =
        removed.iter().map(|r| r.path.clone()).collect();
    if let Some(assets) = &mut index.assets {
        assets.retain(|a| !remove_paths.contains(&a.path));
    }

    // Kept: in both
    let kept_paths: std::collections::BTreeSet<String> =
        disk_paths.intersection(&indexed_paths).cloned().collect();
    let kept_count = kept_paths.len() as u64;

    // Refreshed: only when --refresh-metadata is set
    let refreshed = if refresh_metadata {
        let results = update_all(project_path)?;
        if results.is_empty() {
            None
        } else {
            Some(results)
        }
    } else {
        None
    };

    // Sort all assets by name
    if let Some(assets) = &mut index.assets {
        assets.sort_by(|a, b| a.name.cmp(&b.name));
    }

    if !dry_run {
        save_index(&index, project_path)?;
    }

    Ok(AssetIndexReport { added, removed, kept_count, refreshed })
}
