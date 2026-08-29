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

/// One file found on disk under a project's `sources/` layout during a scan.
/// Reused by both the legacy [`reconcile`] and the Lance-mode adoption pass
/// (spec 075 US2) so there is exactly one directory walk per project.
#[derive(Debug, Clone)]
pub(crate) struct DiskSource {
    pub(crate) path: String,
    pub(crate) source_kind: Option<SourceKind>,
}

fn source_kind_dir_name(source_kind: &SourceKind) -> String {
    match source_kind {
        SourceKind::Yuque => "yuque".to_string(),
        SourceKind::Meeting => "meeting".to_string(),
        SourceKind::Misc => "misc".to_string(),
        SourceKind::Other(value) => value.clone(),
    }
}

pub(crate) fn scan_disk_sources(project_path: &Path) -> Result<Vec<DiskSource>> {
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

    // Bug #7 fix: also scan top-level files and any unrecognised subdirectories
    // under sources/ so that files placed directly there (or in custom layouts)
    // are not invisible to the scanner. This prevents full-index wipe when a
    // rescan cannot surface previously-indexed paths. Must recurse — a
    // shallow scan skips directories entirely, so a file nested inside an
    // unrecognised subdirectory (spec 075 edge case) was invisible to the
    // scan and reported neither present nor missing.
    for abs_path in scan_recursive_dir(&sources_dir)? {
        let portable = util::rel_posix_path(project_path, &abs_path)?;
        // Skip files already covered by the named scans above.
        if files.iter().any(|f| f.path == portable) {
            continue;
        }
        files.push(DiskSource { path: portable, source_kind: None });
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
        // Spec 075 FR-014/FR-016: an entry whose recorded file went missing is
        // *reported* in `removed` but stays in the index — only the explicit
        // removal command (`mf source remove`) may drop it (I-1). The
        // pre-existing entry is spliced back verbatim so its timestamps and
        // uninterpreted fields survive untouched.
        for (name, _ip, _kind) in &indexed_files {
            if let Some(orig) = by_name.get(name.as_str()) {
                new_sources.push((*orig).clone());
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
                extra: Default::default(),
            });
        }

        // Spec 075 FR-018: entries the operation did not change must keep
        // their existing order — pre-existing entries stay in index order
        // (url entries first, then file entries as indexed) and freshly
        // scanned files are appended after. Sorting the merged list here
        // reordered untouched entries on every run, even a no-op.
        index.sources = Some(new_sources);
        index::save(project_path, &index)?;
    }

    added.sort_by(|a, b| a.name.cmp(&b.name));
    removed.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(SourceIndexReport { added, removed, kept_count, dry_run: dry_run_value })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconcile_retains_existing_and_adds_disk_sources() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path();
        std::fs::write(project.join("mind.yaml"), "schema_version: '1'\n").unwrap();
        std::fs::create_dir_all(project.join("sources/file")).unwrap();
        std::fs::write(project.join("sources/file/kept.md"), "kept\n").unwrap();
        std::fs::write(project.join("sources/file/added.md"), "added\n").unwrap();
        std::fs::write(
            project.join("mind-index.yaml"),
            "schema_version: '1'\nsources:\n  - name: kept\n    type: file\n    path: sources/file/kept.md\n    url: null\n    tags: []\n    added_at: old\n    updated_at: old\n",
        )
        .unwrap();

        let report = reconcile(project, false).unwrap();
        assert_eq!(report.kept_count, 1);
        assert_eq!(report.added.len(), 1);
        assert!(report.removed.is_empty());
        let sources = index::load(project).unwrap().sources.unwrap();
        assert_eq!(sources.len(), 2);
        assert!(sources.iter().any(|source| source.name == "kept" && source.added_at == "old"));
    }

    /// Spec 075 FR-014/FR-016: an entry whose file went missing is reported
    /// in `removed` but must survive the rewrite (only `source remove` may
    /// drop it), and FR-011 requires uninterpreted fields (`provenance_note`)
    /// to round-trip instead of being dropped by the typed rebuild.
    #[test]
    fn reconcile_keeps_missing_file_entries_and_uninterpreted_fields() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path();
        std::fs::write(project.join("mind.yaml"), "schema_version: '1'\n").unwrap();
        std::fs::create_dir_all(project.join("sources/file")).unwrap();
        std::fs::write(project.join("sources/file/kept.md"), "kept\n").unwrap();
        std::fs::write(
            project.join("mind-index.yaml"),
            "schema_version: '1'\nsources:\n  sources/kept.md:\n    name: kept\n    type: file\n    url: null\n    path: sources/file/kept.md\n    tags: []\n    added_at: old\n    updated_at: old\n    provenance_note: keep-me-verbatim\n  sources/ghost.md:\n    name: ghost\n    type: file\n    url: null\n    path: sources/file/ghost.md\n    tags: []\n    added_at: gone\n    updated_at: gone\n",
        )
        .unwrap();

        let report = reconcile(project, false).unwrap();
        assert_eq!(report.removed.len(), 1, "the missing file must be reported");
        assert_eq!(report.removed[0].name, "ghost");

        let raw = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
        assert!(raw.contains("sources/file/ghost.md"), "ghost entry must stay in the index: {raw}");
        assert!(raw.contains("added_at: gone"), "ghost timestamps must be untouched: {raw}");
        assert!(raw.contains("provenance_note: keep-me-verbatim"), "extras must round-trip: {raw}");

        let sources = index::load(project).unwrap().sources.unwrap();
        assert_eq!(sources.len(), 2, "kept + ghost, nothing dropped");
        let ghost = sources.iter().find(|s| s.name == "ghost").unwrap();
        assert_eq!(ghost.added_at, "gone");
        let kept = sources.iter().find(|s| s.name == "kept").unwrap();
        assert_eq!(kept.extra.get("provenance_note").and_then(|v| v.as_str()), Some("keep-me-verbatim"));
    }

    /// Spec 075 FR-018: entries the operation did not change must not be
    /// reordered — not on a no-op run and not when new files are adopted.
    #[test]
    fn reconcile_keeps_entry_order_and_appends_new_ones() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path();
        std::fs::write(project.join("mind.yaml"), "schema_version: '1'\n").unwrap();
        std::fs::create_dir_all(project.join("sources/file")).unwrap();
        std::fs::write(project.join("sources/file/zzz.md"), "z\n").unwrap();
        std::fs::write(project.join("sources/file/aaa.md"), "a\n").unwrap();
        // Registered zzz before aaa — deliberately anti-alphabetical.
        std::fs::write(
            project.join("mind-index.yaml"),
            "schema_version: '1'\nsources:\n  sources/zzz.md:\n    name: zzz\n    type: file\n    url: null\n    path: sources/file/zzz.md\n    tags: []\n    added_at: t\n    updated_at: t\n  sources/aaa.md:\n    name: aaa\n    type: file\n    url: null\n    path: sources/file/aaa.md\n    tags: []\n    added_at: t\n    updated_at: t\n",
        )
        .unwrap();

        // No-op run: zero diff, order untouched.
        let report = reconcile(project, false).unwrap();
        assert!(report.added.is_empty() && report.removed.is_empty());
        let order = |index: &crate::model::index::IndexFile| -> Vec<String> {
            index.sources.as_ref().unwrap().iter().map(|s| s.name.clone()).collect()
        };
        let index = index::load(project).unwrap();
        assert_eq!(order(&index), vec!["zzz", "aaa"], "no-op must not reorder: {index:?}");

        // A new on-disk file is adopted after the existing entries.
        std::fs::write(project.join("sources/file/mmm.md"), "m\n").unwrap();
        let report = reconcile(project, false).unwrap();
        assert_eq!(report.added.len(), 1);
        let index = index::load(project).unwrap();
        assert_eq!(order(&index), vec!["zzz", "aaa", "mmm"], "new entries append after existing: {index:?}");
    }
}
