use std::path::Path;
use std::path::PathBuf;

use chrono::Utc;

use super::infer_kind_from_path;
use crate::error::{MfError, Result};
use crate::model::source::{FileKind, Source, SourceIndexEntry, SourceIndexReport, SourceKind};
use crate::service::config as config_svc;
use crate::service::index;
use crate::service::util;

/// Shared set of filenames to skip during directory scans.
fn is_skipped_filename(name: &str) -> bool {
    matches!(name, ".DS_Store" | ".gitkeep" | "Thumbs.db")
}

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
            continue;
        }
        if ft.is_file() || ft.is_symlink() {
            entries.push(entry.path());
        }
    }
    entries.sort();
    Ok(entries)
}

fn scan_recursive_dir(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut entries = Vec::new();
    if !dir.exists() {
        return Ok(entries);
    }
    let walker = walkdir::WalkDir::new(dir).follow_links(true).sort_by(|a, b| a.file_name().cmp(b.file_name()));
    for result in walker {
        let entry = result.map_err(|e| MfError::Internal(anyhow::anyhow!("filesystem walk error: {e}")))?;
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

#[derive(Debug, Clone)]
struct DiskSource {
    path: String,
    source_kind: Option<SourceKind>,
}

fn source_kind_dir_name(source_kind: &SourceKind) -> &'static str {
    match source_kind {
        SourceKind::Yuque => "yuque",
        SourceKind::Meeting => "meeting",
        SourceKind::Misc => "misc",
    }
}

fn scan_disk_sources(project_path: &Path) -> Result<Vec<DiskSource>> {
    let layout = config_svc::effective_layout(project_path)?;
    let sources_dir = project_path.join(&layout.sources);
    let mut files = Vec::new();

    let pdf_dir = sources_dir.join("pdf");
    for abs_path in scan_shallow_dir(&pdf_dir)? {
        let portable = util::rel_posix_path(project_path, &abs_path)?;
        files.push(DiskSource { path: portable, source_kind: None });
    }

    let file_dir = sources_dir.join("file");
    for abs_path in scan_recursive_dir(&file_dir)? {
        let portable = util::rel_posix_path(project_path, &abs_path)?;
        files.push(DiskSource { path: portable, source_kind: None });
    }

    for source_kind in [SourceKind::Yuque, SourceKind::Meeting, SourceKind::Misc] {
        let source_kind_dir = sources_dir.join(source_kind_dir_name(&source_kind));
        for abs_path in scan_recursive_dir(&source_kind_dir)? {
            let portable = util::rel_posix_path(project_path, &abs_path)?;
            files.push(DiskSource { path: portable, source_kind: Some(source_kind.clone()) });
        }
    }

    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

/// Reconcile disk files with the index. Returns a report of added/removed/kept.
pub fn reconcile(project_path: &Path, dry_run: bool) -> Result<SourceIndexReport> {
    let layout = config_svc::effective_layout(project_path)?;
    let sources_dir = project_path.join(&layout.sources);
    if !sources_dir.exists() {
        return Err(MfError::usage(
            format!("project has no {}/ directory at '{}'", layout.sources, sources_dir.display()),
            Some("use `mf project lint --fix` to create missing directories".to_string()),
        ));
    }

    let mut index = index::load(project_path)?;
    let index_sources = index.sources.unwrap_or_default();

    let mut url_kept: Vec<Source> = Vec::new();
    let mut indexed_files: Vec<(String, String, FileKind)> = Vec::new();
    for s in &index_sources {
        match s.kind {
            FileKind::Auto | FileKind::Rss | FileKind::Web => {
                url_kept.push(s.clone());
            }
            FileKind::Pdf | FileKind::File => {
                let p = s.path.as_ref().ok_or_else(|| {
                    MfError::Internal(anyhow::anyhow!("file-type source '{}' has no path field", s.name))
                })?;
                indexed_files.push((s.name.clone(), p.clone(), s.kind.clone()));
            }
        }
    }

    let disk_files = scan_disk_sources(project_path)?;
    let disk_sources: std::collections::BTreeMap<String, Option<SourceKind>> =
        disk_files.into_iter().map(|source| (source.path, source.source_kind)).collect();
    let disk_paths: std::collections::BTreeSet<String> = disk_sources.keys().cloned().collect();

    let mut added: Vec<SourceIndexEntry> = Vec::new();
    let mut kept_file_paths: Vec<String> = Vec::new();
    for disk_path in &disk_paths {
        let in_index = indexed_files.iter().any(|(_, ip, _)| ip == disk_path);
        if in_index {
            kept_file_paths.push(disk_path.clone());
        } else {
            let p = Path::new(disk_path);
            let name = match p.file_stem().and_then(|s| s.to_str()) {
                Some(stem) => stem.to_string(),
                None => continue,
            };
            let kind = infer_kind_from_path(p);
            added.push(SourceIndexEntry { name: name.clone(), kind, path: disk_path.clone() });
        }
    }

    let mut removed: Vec<SourceIndexEntry> = Vec::new();
    for (name, ip, kind) in &indexed_files {
        if !disk_paths.contains(ip) {
            removed.push(SourceIndexEntry { name: name.clone(), kind: kind.clone(), path: ip.clone() });
        }
    }

    let kept_count = (url_kept.len() as u64) + kept_file_paths.len() as u64;
    let dry_run_value = dry_run;

    if !dry_run {
        let mut new_sources: Vec<Source> = url_kept.clone();

        let by_name: std::collections::HashMap<&str, &Source> =
            index_sources.iter().map(|s| (s.name.as_str(), s)).collect();
        for (name, ip, _kind) in &indexed_files {
            if kept_file_paths.contains(ip) {
                if let Some(orig) = by_name.get(name.as_str()) {
                    new_sources.push((*orig).clone());
                }
            }
        }

        let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        for entry in &added {
            new_sources.push(Source {
                name: entry.name.clone(),
                kind: entry.kind.clone(),
                source_kind: disk_sources.get(&entry.path).cloned().flatten(),
                url: None,
                path: Some(entry.path.clone()),
                tags: vec![],
                added_at: now.clone(),
                updated_at: now.clone(),
            });
        }

        new_sources.sort_by(|a, b| a.name.cmp(&b.name));

        index.sources = Some(new_sources);
        index::save(project_path, &index)?;
    }

    added.sort_by(|a, b| a.name.cmp(&b.name));
    removed.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(SourceIndexReport { added, removed, kept_count, dry_run: dry_run_value })
}
