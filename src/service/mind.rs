//! Canonical mind/output discovery. The disk owns output content; mind.yaml
//! owns registrations and defaults, including minds that have no outputs.
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::error::{MfError, Result};
use crate::model::config::MindConfig;
use crate::model::mind::{ContentLayout, Mind, Output};
use crate::service::{config, util};

pub fn config_for(project: &Path) -> Result<MindConfig> {
    config::load_project(project, Some(project))?.ok_or_else(|| MfError::not_found("mind.yaml missing", None))
}

pub fn validate_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.starts_with('.')
        || name.ends_with(".md")
        || name.contains(['/', '\\', ':'])
        || name.trim() != name
        || name.chars().any(char::is_control)
    {
        return Err(MfError::usage(format!("invalid mind/output name: {name:?}"), None));
    }
    Ok(())
}

pub fn checked_path(project: &Path, relative: &str) -> Result<PathBuf> {
    let rel = Path::new(relative);
    if rel.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(MfError::usage(format!("unsafe content path: {relative}"), None));
    }
    let mut path = project.to_path_buf();
    for part in rel.components() {
        path.push(part);
        if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(MfError::usage(format!("symlink content path refused: {}", path.display()), None));
        }
    }
    Ok(path)
}

pub(crate) fn reserved(config: &MindConfig) -> BTreeSet<String> {
    let mut names: BTreeSet<String> = [
        "build",
        "outputs",
        "docs",
        "sources",
        "assets",
        "prompts",
        "thinking",
        "templates",
        "cache",
        "target",
        "node_modules",
        "vendor",
        "dist",
        "_archived",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    if let Some(layout) = &config.layout {
        for value in [&layout.articles, &layout.sources, &layout.assets, &layout.templates, &layout.build_output]
            .into_iter()
            .flatten()
        {
            if let Some(first) = value.split('/').next() {
                names.insert(first.to_string());
            }
        }
    }
    names.extend(config.reserved_roots.iter().cloned());
    names
}

fn children(project: &Path, relative: &str) -> Result<Vec<(String, bool)>> {
    children_excluding(project, relative, &BTreeSet::new())
}

fn children_excluding(project: &Path, relative: &str, excluded: &BTreeSet<String>) -> Result<Vec<(String, bool)>> {
    let path = if relative.is_empty() { project.to_path_buf() } else { checked_path(project, relative)? };
    if !path.exists() {
        return Ok(vec![]);
    }
    let mut entries = Vec::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || excluded.contains(&name) {
            continue;
        }
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            return Err(MfError::usage(format!("symlink content refused: {}", entry.path().display()), None));
        }
        entries.push((name, kind.is_dir()));
    }
    entries.sort();
    Ok(entries)
}

pub fn output_root(mode: ContentLayout, mind: &str) -> String {
    match mode {
        ContentLayout::Category => format!("outputs/{mind}"),
        ContentLayout::Bundle => format!("{mind}/outputs"),
    }
}

pub fn discover(root: &Path, project: &Path) -> Result<Vec<Mind>> {
    let cfg = config_for(project)?;
    let mode = cfg.content_layout.ok_or_else(|| MfError::usage("project has no content_layout; use a new project with --layout category|bundle; legacy ownership requires explicit migration", None))?;
    let project_identity = util::rel_posix_path(root, project)?;
    if project_identity.contains("::") {
        return Err(MfError::usage("project identity cannot contain ::", None));
    }
    let mut names: BTreeSet<String> = cfg.minds.keys().cloned().collect();
    let mut excluded = reserved(&cfg);
    excluded.extend(child_project_roots(root, project)?);
    if mode == ContentLayout::Bundle {
        for (name, directory) in children_excluding(project, "", &excluded)? {
            if directory && !excluded.contains(&name) {
                names.insert(name);
            }
        }
    } else {
        for role in ["outputs", "prompts", "thinking", "sources", "assets"] {
            for (name, directory) in children(project, role)? {
                if directory {
                    names.insert(name);
                } else if matches!(role, "prompts" | "thinking") && name.ends_with(".md") {
                    names.insert(name.trim_end_matches(".md").to_string());
                }
            }
        }
    }
    let mut folded = BTreeSet::new();
    let mut result = Vec::new();
    for name in names {
        validate_name(&name)?;
        if excluded.iter().any(|n| n.eq_ignore_ascii_case(&name)) || !folded.insert(name.to_lowercase()) {
            return Err(MfError::usage(format!("reserved or case-colliding mind name: {name}"), None));
        }
        let identity = format!("{project_identity}::{name}");
        let registration = cfg.minds.get(&name).cloned().unwrap_or_default();
        let output_root = output_root(mode, &name);
        let mut outputs = BTreeMap::new();
        for (file, directory) in children(project, &output_root)? {
            if !directory && !file.ends_with(".md") {
                continue;
            }
            let output_name = if directory { file.clone() } else { file.trim_end_matches(".md").to_string() };
            validate_name(&output_name)?;
            let path = format!("{output_root}/{file}");
            if directory {
                children(project, &path)?;
            }
            let output_identity = format!("{identity}::{output_name}");
            let output = Output {
                identity: output_identity.clone(),
                project_identity: project_identity.clone(),
                mind_identity: identity.clone(),
                output_identity,
                mind: name.clone(),
                name: output_name.clone(),
                path: path.clone(),
                shape: if directory { "directory" } else { "file" }.into(),
                build_path: format!("build/{name}/{output_name}.md"),
            };
            if outputs.insert(output_name.to_lowercase(), output).is_some() {
                return Err(MfError::ShapeConflict {
                    wanted_shape: "unique output".into(),
                    existing_shape: "same-name output".into(),
                    path: project.join(path),
                });
            }
        }
        let mut roles = BTreeMap::new();
        for (role, category, bundle) in [
            ("prompt", format!("prompts/{name}.md"), format!("{name}/prompt.md")),
            ("thinking", format!("thinking/{name}.md"), format!("{name}/thinking.md")),
            ("sources", format!("sources/{name}"), format!("{name}/sources")),
            ("assets", format!("assets/{name}"), format!("{name}/assets")),
        ] {
            let path = if mode == ContentLayout::Category { category } else { bundle };
            if checked_path(project, &path)?.exists() {
                roles.insert(role.into(), path);
            }
        }
        result.push(Mind {
            identity: identity.clone(),
            project_identity: project_identity.clone(),
            mind_identity: identity,
            title: registration.title.unwrap_or_else(|| name.clone()),
            path: if mode == ContentLayout::Bundle && checked_path(project, &name)?.is_dir() {
                Some(name.clone())
            } else {
                None
            },
            name,
            default_output: registration.default_output,
            roles,
            outputs: outputs.into_values().collect(),
        });
    }
    Ok(result)
}

pub fn resolve_mind(root: &Path, project: &Path, selector: &str) -> Result<Mind> {
    let minds = discover(root, project)?;
    minds
        .into_iter()
        .find(|m| m.identity == selector || m.name == selector)
        .ok_or_else(|| MfError::not_found(format!("mind not found: {selector}"), None))
}

pub fn resolve_output(root: &Path, project: &Path, selector: &str) -> Result<Output> {
    let minds = discover(root, project)?;
    let outputs: Vec<_> = minds.iter().flat_map(|m| m.outputs.iter()).collect();
    if let Some(output) = outputs.iter().find(|o| o.identity == selector || o.path == selector) {
        return Ok((*output).clone());
    }
    if let Some(mind) = minds.iter().find(|m| m.identity == selector || m.name == selector) {
        if mind.name == selector {
            let mut collisions: Vec<_> =
                outputs.iter().copied().filter(|o| o.name == selector && o.mind != mind.name).collect();
            if !collisions.is_empty() {
                collisions.extend(mind.outputs.iter());
                return Err(MfError::SelectionAmbiguous {
                    selector: selector.into(),
                    candidates: std::iter::once(mind.identity.clone())
                        .chain(collisions.iter().map(|o| o.identity.clone()))
                        .collect(),
                });
            }
        }

        if let Some(default) = &mind.default_output {
            return mind.outputs.iter().find(|o| &o.name == default).cloned().ok_or_else(|| {
                MfError::not_found(format!("default output missing: {}::{default}", mind.identity), None)
            });
        }
        return unique_output(&mind.outputs.iter().collect::<Vec<_>>(), selector);
    }
    let matches: Vec<_> =
        outputs.into_iter().filter(|o| o.name == selector || format!("{}::{}", o.mind, o.name) == selector).collect();
    unique_output(&matches, selector)
}

fn unique_output(outputs: &[&Output], selector: &str) -> Result<Output> {
    match outputs {
        [output] => Ok((*output).clone()),
        [] => Err(MfError::not_found(format!("no output for: {selector}"), None)),
        _ => Err(MfError::SelectionAmbiguous {
            selector: selector.into(),
            candidates: outputs.iter().map(|o| o.identity.clone()).collect(),
        }),
    }
}

pub fn project_for(root: &Path, cwd: &Path, selected: Option<&str>, selector: Option<&str>) -> Result<PathBuf> {
    if let Some((project, _)) = selector.and_then(|s| s.split_once("::")) {
        let path = checked_path(root, project)?;
        if path.join("mind.yaml").is_file() {
            if let Some(selected) = selected {
                let explicit = util::resolve_project(root, Some(selected), cwd)?;
                if util::try_canonicalize(&explicit) != util::try_canonicalize(&path) {
                    return Err(MfError::usage("identity conflicts with --project", None));
                }
            }
            return Ok(path);
        }
    }
    util::resolve_project(root, selected, cwd)
}

pub fn save_config_value(project: &Path, value: &serde_yaml::Value) -> Result<()> {
    let text = serde_yaml::to_string(value).map_err(|e| MfError::Internal(e.into()))?;
    util::atomic_write(&project.join("mind.yaml"), &text)
}

pub fn raw_config(project: &Path) -> Result<serde_yaml::Value> {
    let path = project.join("mind.yaml");
    serde_yaml::from_str(&fs::read_to_string(&path)?).map_err(|e| MfError::ParseError {
        kind: "yaml".into(),
        path,
        detail: e.to_string(),
    })
}

pub fn index_outputs(root: &Path, project: &Path, dry_run: bool) -> Result<Vec<Output>> {
    let outputs: Vec<_> = discover(root, project)?.into_iter().flat_map(|m| m.outputs).collect();
    if !dry_run {
        let mut index = crate::service::index::load(project)?;
        let extra = index.extra.get_or_insert_with(Vec::new);
        extra.retain(|(key, _)| key != "outputs");
        extra.push(("outputs".into(), serde_json::to_value(&outputs)?));
        crate::service::index::save(project, &index)?;
    }
    Ok(outputs)
}

pub fn as_article(output: &Output, project: &Path) -> crate::model::article::Article {
    crate::model::article::Article {
        title: output.name.clone(),
        project: util::dir_name(project),
        article_type: crate::model::article::ArticleType::Blog,
        article_path: output.path.clone(),
        status: crate::model::article::ArticleStatus::Draft,
        created_at: String::new(),
        updated_at: String::new(),
        template_origin: None,
    }
}

pub fn validate_mind_name(cfg: &MindConfig, name: &str) -> Result<()> {
    validate_name(name)?;
    if reserved(cfg).iter().any(|n| n.eq_ignore_ascii_case(name)) {
        return Err(MfError::usage(format!("reserved mind name: {name}"), None));
    }
    Ok(())
}

pub fn canonicalize_existing_prefix(path: &Path) -> PathBuf {
    let mut cursor = path.to_path_buf();
    let mut suffix: Vec<std::ffi::OsString> = Vec::new();

    while !cursor.exists() {
        let Some(name) = cursor.file_name() else {
            return util::markdown::normalize_lexical(path);
        };
        suffix.push(name.to_os_string());
        let Some(parent) = cursor.parent() else {
            return util::markdown::normalize_lexical(path);
        };
        cursor = parent.to_path_buf();
    }

    let mut normalized = cursor.canonicalize().unwrap_or(cursor);
    for part in suffix.iter().rev() {
        normalized.push(part);
    }
    util::markdown::normalize_lexical(&normalized)
}

/// Enumerate registered and discoverable projects without assuming projects/.
pub fn project_paths(root: &Path) -> Result<Vec<PathBuf>> {
    let manifest = crate::service::repo::load_manifest(&root.join("minds.yaml"))?;
    let mut paths = BTreeSet::new();
    if root.join("mind.yaml").is_file() {
        paths.insert(root.to_path_buf());
    }
    for entry in &manifest.projects {
        if entry.archived_at.is_none() {
            paths.insert(util::markdown::normalize_lexical(&root.join(entry.path.trim_start_matches("./"))));
        }
    }
    for entry in crate::service::repo::scan_project_dirs(root, &manifest.projects_dir) {
        paths.insert(util::markdown::normalize_lexical(&root.join(entry.path.trim_start_matches("./"))));
    }
    Ok(paths.into_iter().filter(|p| p.join("mind.yaml").is_file()).collect())
}

/// Child projects retain their own ownership when their parent is a project.
pub fn child_project_roots(root: &Path, project: &Path) -> Result<BTreeSet<String>> {
    let mut excluded = BTreeSet::new();
    if !root.join("minds.yaml").exists() {
        return Ok(excluded);
    }
    for child in project_paths(root)? {
        if let Ok(relative) = child.strip_prefix(project)
            && let Some(first) = relative.components().next()
        {
            excluded.insert(first.as_os_str().to_string_lossy().into_owned());
        }
    }
    if root == project {
        let manifest = crate::service::repo::load_manifest(&root.join("minds.yaml"))?;
        if let Some(first) = Path::new(&manifest.projects_dir).components().find(|c| matches!(c, Component::Normal(_)))
        {
            excluded.insert(first.as_os_str().to_string_lossy().into_owned());
        }
    }
    Ok(excluded)
}

pub fn validate_projects(root: &Path) -> Result<()> {
    for project in project_paths(root)? {
        if config_for(&project)?.content_layout.is_some() {
            discover(root, &project)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_excludes_configured_and_hidden_roots_and_preserves_zero_minds() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("mind.yaml"), "schema: '1'\ncontent_layout: bundle\nlayout:\n  templates: guides\n  build_output: generated\nminds:\n  registered: {}\n").unwrap();
        for path in ["guides", "generated", ".tool", "build", "ordinary"] {
            fs::create_dir(temp.path().join(path)).unwrap();
        }
        let minds = discover(temp.path(), temp.path()).unwrap();
        assert_eq!(minds.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["ordinary", "registered"]);
        assert!(minds.iter().all(|m| m.outputs.is_empty()));
        assert!(minds[1].path.is_none());
    }

    #[test]
    fn output_shape_case_and_symlink_collisions_are_not_guessed() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("mind.yaml"), "content_layout: category\nminds:\n  brief: {}\n").unwrap();
        let dir = temp.path().join("outputs/brief");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("summary.md"), "# Summary").unwrap();
        fs::create_dir(dir.join("SUMMARY")).unwrap();
        assert!(matches!(discover(temp.path(), temp.path()), Err(MfError::ShapeConflict { .. })));
    }

    #[cfg(unix)]
    #[test]
    fn symlink_blocks_and_build_parents_are_rejected() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("mind.yaml"), "content_layout: bundle\n").unwrap();
        fs::create_dir_all(temp.path().join("brief/outputs/design")).unwrap();
        fs::write(temp.path().join("secret.md"), "secret").unwrap();
        std::os::unix::fs::symlink(temp.path().join("secret.md"), temp.path().join("brief/outputs/design/01-title.md"))
            .unwrap();
        assert!(discover(temp.path(), temp.path()).is_err());
        std::os::unix::fs::symlink(temp.path().join("brief"), temp.path().join("build")).unwrap();
        assert!(checked_path(temp.path(), "build/design.md").is_err());
    }
}
