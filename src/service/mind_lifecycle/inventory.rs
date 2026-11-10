//! Owned roots, independent of output count. Unknown ownership is an error;
//! imported_by and display titles never establish ownership.
use super::transaction::{Entry, Snapshot};
use crate::error::{MfError, Result};
use crate::model::mind::ContentLayout;
use crate::service::mind;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedRoot {
    pub mind: String,
    pub kind: String,
    pub path: String,
    pub file: bool,
}

fn reject(message: impl Into<String>) -> MfError {
    MfError::Lifecycle { code: "ownership_unresolved", message: message.into() }
}

pub fn destination(mode: ContentLayout, owner: &str, kind: &str, file: bool) -> String {
    match (mode, kind) {
        (ContentLayout::Category, "prompt") => format!("prompts/{owner}.md"),
        (ContentLayout::Category, "thinking") => format!("thinking/{owner}.md"),
        (ContentLayout::Category, _) => format!("{kind}/{owner}{}", if file { ".md" } else { "" }),
        (ContentLayout::Bundle, _) => format!("{owner}/{kind}{}", if file { ".md" } else { "" }),
    }
}

pub(crate) fn valid_kind(kind: &str) -> bool {
    !kind.is_empty()
        && kind.split('-').all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_alphabetic() || b.is_ascii_digit()))
}

/// Paths are project-relative; callers supply a complete snapshot so hidden
/// placeholders can be retained even though they are not additional items.
pub fn collect(root: &Path, project: &Path, files: &Snapshot) -> Result<Vec<OwnedRoot>> {
    let config = mind::config_for(project)?;
    let mode = config.content_layout.ok_or_else(|| reject("legacy project has no explicit layout"))?;
    let minds = mind::discover(root, project)?;
    let names: BTreeSet<_> = minds.iter().map(|m| m.name.as_str()).collect();
    let mut excluded: BTreeSet<String> =
        ["build", "docs", "templates", "cache", "target", "node_modules", "vendor", "dist", "_archived"]
            .into_iter()
            .map(String::from)
            .collect();
    excluded.extend(mind::child_project_roots(root, project)?);
    if mode == ContentLayout::Category {
        excluded.extend(config.reserved_roots.iter().cloned());
    }
    if let Some(layout) = &config.layout {
        for (configured, standard) in
            [(&layout.articles, "docs"), (&layout.sources, "sources"), (&layout.assets, "assets")]
        {
            if let Some(path) = configured
                && path != standard
                && files.iter().any(|(p, entry)| {
                    Path::new(p).starts_with(path)
                        && matches!(entry, Entry::File { .. })
                        && !p.split('/').any(|part| part.starts_with('.'))
                })
            {
                return Err(reject(format!("configured legacy core root {path}: explicit ownership mapping required")));
            }
        }
        for path in [&layout.articles, &layout.sources, &layout.assets, &layout.templates, &layout.build_output]
            .into_iter()
            .flatten()
        {
            excluded.insert(path.split('/').next().unwrap().into());
        }
    }
    // Configured nonstandard source/asset roots cannot silently become additional.
    for role in ["outputs", "prompts", "prompt", "thinking", "sources", "assets"] {
        excluded.remove(role);
    }
    let mut found = Vec::new();
    for (path, entry) in files {
        let parts: Vec<_> = path.split('/').collect();
        if parts.iter().any(|p| p.starts_with('.')) {
            continue;
        }
        let file = matches!(entry, Entry::File { .. });
        match mode {
            ContentLayout::Category => {
                if parts.len() != 2 || excluded.contains(parts[0]) {
                    continue;
                }
                let kind = match parts[0] {
                    "prompts" => "prompt",
                    other => other,
                };
                let owner = if file { parts[1].strip_suffix(".md").unwrap_or(parts[1]) } else { parts[1] };
                if !names.contains(owner) {
                    return Err(MfError::ContentConflict {
                        code: "ownership_unresolved",
                        message: format!("{path}: no unique known mind owner"),
                        candidates: minds.iter().map(|m| m.identity.clone()).collect(),
                    });
                }
                if !valid_kind(kind) || (file && !parts[1].ends_with(".md")) {
                    return Err(reject(format!("{path}: unsupported role/additional shape")));
                }
                validate_role(kind, file, path)?;
                found.push(OwnedRoot { mind: owner.into(), kind: kind.into(), path: path.clone(), file });
            }
            ContentLayout::Bundle => {
                if parts.len() != 2 || !names.contains(parts[0]) {
                    continue;
                }
                let kind = if file {
                    parts[1].strip_suffix(".md").ok_or_else(|| reject(format!("{path}: unknown top-level file")))?
                } else {
                    parts[1]
                };
                if excluded.contains(kind) {
                    continue;
                }
                if !valid_kind(kind) {
                    return Err(reject(format!("{path}: unsupported additional kind")));
                }
                validate_role(kind, file, path)?;
                found.push(OwnedRoot { mind: parts[0].into(), kind: kind.into(), path: path.clone(), file });
            }
        }
    }
    let mut kinds: BTreeMap<String, String> = BTreeMap::new();
    for item in &found {
        if mode == ContentLayout::Category
            && let Some(previous) = kinds.insert(item.kind.to_lowercase(), item.kind.clone())
            && previous != item.kind
        {
            return Err(MfError::ContentConflict {
                code: "path_conflict",
                message: "category case collision".into(),
                candidates: found
                    .iter()
                    .filter(|i| i.kind.eq_ignore_ascii_case(&item.kind))
                    .map(|i| i.path.clone())
                    .collect(),
            });
        }
    }
    let mut keys = BTreeSet::new();
    for item in &found {
        if !keys.insert((item.mind.to_lowercase(), item.kind.to_lowercase())) {
            return Err(MfError::ContentConflict {
                code: "path_conflict",
                message: "file/directory or case collision".into(),
                candidates: found
                    .iter()
                    .filter(|other| {
                        other.mind.eq_ignore_ascii_case(&item.mind) && other.kind.eq_ignore_ascii_case(&item.kind)
                    })
                    .map(|other| other.path.clone())
                    .collect(),
            });
        }
    }
    // Legacy articles are not assigned to a mind just because their names match.
    if files.iter().any(|(p, e)| {
        p.starts_with("docs/") && matches!(e, Entry::File { .. }) && !p.split('/').any(|s| s.starts_with('.'))
    }) {
        return Err(reject("legacy docs contain files; explicit ownership mapping required"));
    }
    Ok(found)
}

fn validate_role(kind: &str, file: bool, path: &str) -> Result<()> {
    for core in ["prompt", "thinking", "outputs", "sources", "assets"] {
        if kind != core && kind.eq_ignore_ascii_case(core) {
            return Err(MfError::ContentConflict {
                code: "path_conflict",
                message: "core role case collision".into(),
                candidates: vec![path.into(), core.into()],
            });
        }
    }
    if (matches!(kind, "prompt" | "thinking") && !file) || (matches!(kind, "outputs" | "sources" | "assets") && file) {
        return Err(reject(format!("{path}: invalid core role shape")));
    }
    Ok(())
}

/// Map all core roles and additional roots; whole bundle moves are handled by
/// the planner to retain otherwise unclassified hidden files in the container.
pub fn mapping(items: &[OwnedRoot], mode: ContentLayout, names: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    items
        .iter()
        .filter_map(|item| {
            names.get(&item.mind).map(|owner| (item.path.clone(), destination(mode, owner, &item.kind, item.file)))
        })
        .collect()
}

/// Read-only discovery needs structure, not the bytes of tool caches or other
/// projects. The transaction planner still takes its complete protected snapshot.
pub fn scan(root: &Path, project: &Path) -> Result<Vec<OwnedRoot>> {
    let config = mind::config_for(project)?;
    let mut ignored: BTreeSet<String> =
        ["build", "docs", "templates", "cache", "target", "node_modules", "vendor", "dist", "_archived"]
            .into_iter()
            .map(String::from)
            .collect();
    if let Some(layout) = &config.layout {
        for path in [&layout.templates, &layout.build_output].into_iter().flatten() {
            ignored.insert(path.split('/').next().unwrap().into());
        }
    }
    let children = mind::child_project_roots(root, project)?;
    let bundle = config.content_layout == Some(ContentLayout::Bundle);
    let mut files = Snapshot::new();
    for entry in walkdir::WalkDir::new(project).follow_links(false).into_iter().filter_entry(|e| {
        if e.depth() == 0 {
            return true;
        }
        let name = e.file_name().to_string_lossy();
        if name.starts_with('.') {
            return false;
        }
        if e.depth() == 1
            && (children.contains(name.as_ref())
                || ignored.contains(name.as_ref())
                || (config.reserved_roots.iter().any(|r| r == name.as_ref())
                    && !matches!(name.as_ref(), "outputs" | "prompts" | "thinking" | "sources" | "assets")))
        {
            return false;
        }
        !(bundle && e.depth() == 2 && ignored.contains(name.as_ref()))
    }) {
        let entry = entry.map_err(|e| MfError::Lifecycle { code: "inventory_failed", message: e.to_string() })?;
        if entry.depth() == 0 {
            continue;
        }
        let path =
            entry.path().strip_prefix(project).unwrap().to_str().ok_or_else(|| reject("non UTF-8 path"))?.to_string();
        let kind = entry.file_type();
        let value = if kind.is_file() {
            Entry::File { bytes: vec![], mode: 0 }
        } else if kind.is_dir() {
            Entry::Directory { mode: 0 }
        } else {
            return Err(MfError::Lifecycle {
                code: "unsafe_path",
                message: format!("non-regular content path: {path}"),
            });
        };
        files.insert(path, value);
    }
    collect(root, project, &files)
}

#[cfg(test)]
mod tests {
    use super::super::transaction::snapshot;
    use super::*;
    use std::fs;

    fn fixture(mode: &str, paths: &[&str]) -> (tempfile::TempDir, Snapshot) {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("mind.yaml"),
            format!("content_layout: {mode}\nminds:\n  brief: {{}}\n  empty: {{}}\n"),
        )
        .unwrap();
        for path in paths {
            let path = dir.path().join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "data").unwrap();
        }
        let watches = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        let files = snapshot(dir.path(), &watches).unwrap();
        (dir, files)
    }

    #[test]
    fn all_roles_outputs_and_additional_have_bidirectional_mapping() {
        let (dir, files) = fixture(
            "category",
            &[
                "outputs/brief/design.md",
                "outputs/brief/summary/01.md",
                "prompts/brief.md",
                "thinking/brief.md",
                "sources/brief/evidence.pdf",
                "assets/brief/img.png",
                "reviews/brief.md",
                "evals/brief/raw.json",
                "evals/brief/.keep",
            ],
        );
        let items = collect(dir.path(), dir.path(), &files).unwrap();
        assert_eq!(items.len(), 7);
        let names = BTreeMap::from([("brief".into(), "brief".into())]);
        let forward = mapping(&items, ContentLayout::Bundle, &names);
        assert_eq!(forward["reviews/brief.md"], "brief/reviews.md");
        assert_eq!(forward["prompts/brief.md"], "brief/prompt.md");
        for item in items {
            assert_eq!(destination(ContentLayout::Category, &item.mind, &item.kind, item.file), item.path);
        }
    }

    #[test]
    fn configured_legacy_core_roots_are_not_silently_skipped() {
        for field in ["articles", "sources", "assets"] {
            let (dir, files) = fixture("category", &["evidence/brief/note.md"]);
            fs::write(
                dir.path().join("mind.yaml"),
                format!("content_layout: category\nminds:\n  brief: {{}}\nlayout:\n  {field}: evidence\n"),
            )
            .unwrap();
            let err = collect(dir.path(), dir.path(), &files).unwrap_err();
            assert_eq!(err.kind(), "ownership_unresolved");
            assert!(err.to_string().contains("configured legacy core root"));
        }
    }

    #[test]
    fn rejects_unknown_ownership_legacy_articles_and_shape_collisions() {
        for (mode, paths) in [
            ("category", vec!["reviews/unknown.md"]),
            ("category", vec!["sources/loose.pdf"]),
            ("category", vec!["docs/brief.md"]),
            ("category", vec!["reviews/brief.md", "reviews/brief/detail.md"]),
            ("bundle", vec!["brief/reviews.md", "brief/reviews/detail.md"]),
            ("bundle", vec!["brief/sources.md"]),
        ] {
            let (dir, files) = fixture(mode, &paths);
            assert!(collect(dir.path(), dir.path(), &files).is_err(), "{mode} {paths:?}");
        }
    }
}
