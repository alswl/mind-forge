//! Complete, read-only lifecycle planning over repository files.
use super::{
    inventory,
    references::{self, Moves},
    transaction::{self, Entry, Plan, Snapshot},
};
use crate::error::{MfError, Result};
use crate::model::mind::ContentLayout;
use crate::service::{mind, util};
use serde_yaml::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Debug)]
pub enum Operation {
    Convert(ContentLayout),
    AdditionalRename { identity: String, destination: String },
    AdditionalRemove { identity: String },
    MindRename { identity: String, name: String },
    MindMove { identity: String, project: String },
    MindRemove { identity: String },
    OutputRename { identity: String, name: String },
    OutputMove { identity: String, mind: String, project: String },
    OutputRemove { identity: String },
}

fn reject(code: &'static str, message: impl Into<String>) -> MfError {
    MfError::Lifecycle { code, message: message.into() }
}
fn yaml_error(e: serde_yaml::Error) -> MfError {
    reject("invalid_metadata", e.to_string())
}
fn raw(files: &Snapshot, path: &str) -> Result<Value> {
    match files.get(path) {
        Some(Entry::File { bytes, .. }) => serde_yaml::from_slice(bytes).map_err(yaml_error),
        None => Ok(Value::Mapping(Default::default())),
        _ => Err(reject("path_conflict", path)),
    }
}
fn put(files: &mut Snapshot, path: &str, value: &Value) -> Result<()> {
    let bytes = serde_yaml::to_string(value).map_err(yaml_error)?.into_bytes();
    let mode = match files.get(path) {
        Some(Entry::File { mode, .. }) => *mode,
        _ => 0o644,
    };
    files.insert(path.into(), Entry::File { bytes, mode });
    Ok(())
}
fn register(raw: &mut Value, name: &str, registration: Value) {
    if !raw["minds"].is_mapping() {
        raw["minds"] = Value::Mapping(Default::default());
    }
    raw["minds"][name] = registration;
}
fn registration(config: &Value, owner: &crate::model::mind::Mind) -> Value {
    let mut item = config["minds"][&owner.name].clone();
    if !item.is_mapping() {
        item = Value::Mapping(Default::default());
    }
    item["title"] = owner.title.clone().into();
    item
}
fn remove_registration(raw: &mut Value, name: &str) {
    if let Some(map) = raw["minds"].as_mapping_mut() {
        map.remove(Value::String(name.into()));
    }
}
fn identity_map(identity: &str, maps: &BTreeMap<String, Option<String>>) -> Option<String> {
    let item = maps
        .iter()
        .filter(|(old, _)| identity == *old || identity.starts_with(&format!("{old}::")))
        .max_by_key(|(old, _)| old.len());
    match item {
        None => Some(identity.into()),
        Some((old, new)) => new.as_ref().map(|n| format!("{n}{}", &identity[old.len()..])),
    }
}

fn prefix(project: &str) -> String {
    if project.is_empty() { String::new() } else { format!("{project}/") }
}
fn scoped(project: &str, path: &str) -> String {
    format!("{}{path}", prefix(project))
}

pub fn plan(root: &Path, project: &Path, operation: Operation) -> Result<Plan> {
    transaction::check_recovery(root)?;
    let project_id = util::rel_posix_path(root, project)?;
    let config = mind::config_for(project)?;
    let mode = config.content_layout.ok_or_else(|| reject("ownership_unresolved", "explicit layout required"))?;
    let watches = vec![".".into()];
    let before = transaction::snapshot(root, &watches)?;
    let project_files = before
        .iter()
        .filter_map(|(p, e)| p.strip_prefix(&prefix(&project_id)).map(|p| (p.into(), e.clone())))
        .collect();
    let owned = inventory::collect(root, project, &project_files)?;
    let mut after = before.clone();
    let mut configs = BTreeMap::from([(project_id.clone(), raw(&before, &scoped(&project_id, "mind.yaml"))?)]);
    let mut moves = Moves::new();
    let mut identities = BTreeMap::new();
    let mut touched = BTreeSet::from([project_id.clone()]);
    let mut cleanup = BTreeSet::new();
    let minds = mind::discover(root, project)?;
    let op_name = format!("{operation:?}");
    match &operation {
        Operation::AdditionalRename { identity, .. } | Operation::AdditionalRemove { identity } => {
            let items: Vec<_> = owned
                .iter()
                .filter(|i| crate::service::additional::is_additional(i))
                .map(|i| crate::service::additional::from_owned(&project_id, i))
                .collect();
            let item = crate::service::additional::select(&items, identity)?;
            let target = if let Operation::AdditionalRename { destination, .. } = &operation {
                crate::service::additional::rename_path(root, project, &item, destination, &items)?;
                Some(scoped(&project_id, destination))
            } else {
                None
            };
            moves.insert(scoped(&project_id, &item.path), target);
        }
        Operation::Convert(target) => {
            if *target == mode {
                return Err(reject("layout_unchanged", "project already has requested layout"));
            }
            let names = minds.iter().map(|m| (m.name.clone(), m.name.clone())).collect();
            for (old, new) in inventory::mapping(&owned, *target, &names) {
                moves.insert(scoped(&project_id, &old), Some(scoped(&project_id, &new)));
            }
            for owner in &minds {
                let cfg = configs.get_mut(&project_id).unwrap();
                register(cfg, &owner.name, registration(&before_config(&before, &project_id)?, owner));
                if mode == ContentLayout::Bundle {
                    cleanup.insert(scoped(&project_id, &owner.name));
                }
                if *target == ContentLayout::Bundle {
                    let path = scoped(&project_id, &owner.name);
                    if before.contains_key(&path) && !placeholder_container(&before, &path) {
                        return Err(reject("path_conflict", path));
                    }
                    after.entry(path).or_insert(Entry::Directory { mode: 0o755 });
                }
            }
            if *target == ContentLayout::Bundle {
                let mut reserved: BTreeSet<String> = config.reserved_roots.iter().cloned().collect();
                for item in &owned {
                    let role = item.path.split('/').next().unwrap();
                    reserved.insert(role.into());
                }
                configs.get_mut(&project_id).unwrap()["reserved_roots"] =
                    serde_yaml::to_value(reserved).map_err(yaml_error)?;
            }
            if *target == ContentLayout::Category {
                let role_roots: BTreeSet<_> = owned
                    .iter()
                    .map(|i| {
                        inventory::destination(*target, &i.mind, &i.kind, i.file).split('/').next().unwrap().to_string()
                    })
                    .collect();
                configs.get_mut(&project_id).unwrap()["reserved_roots"] = serde_yaml::to_value(
                    config.reserved_roots.iter().filter(|r| !role_roots.contains(*r)).collect::<Vec<_>>(),
                )
                .map_err(yaml_error)?;
            }
            configs.get_mut(&project_id).unwrap()["content_layout"] =
                serde_yaml::to_value(target).map_err(yaml_error)?;
        }
        Operation::MindRename { identity, .. }
        | Operation::MindMove { identity, .. }
        | Operation::MindRemove { identity } => {
            let owner = mind::resolve_mind(root, project, identity)?;
            let (dest_project, new_name) = match &operation {
                Operation::MindRename { name, .. } => (Some(project_id.clone()), name.clone()),
                Operation::MindMove { project: selector, .. } => {
                    let dest = util::resolve_project(root, Some(selector), root)?;
                    (Some(util::rel_posix_path(root, &dest)?), owner.name.clone())
                }
                _ => (None, owner.name.clone()),
            };
            let mut target_mode = mode;
            if let Some(dest) = &dest_project {
                if dest == &project_id && new_name == owner.name {
                    return Err(reject("path_conflict", "source and target are identical"));
                }
                let target = root.join(dest);
                let cfg = mind::config_for(&target)?;
                mind::validate_mind_name(&cfg, &new_name)?;
                if mind::child_project_roots(root, &target)?.contains(&new_name) {
                    return Err(reject("path_conflict", "mind name conflicts with child project container"));
                }
                if mind::discover(root, &target)?.iter().any(|m| m.name.eq_ignore_ascii_case(&new_name)) {
                    return Err(reject("path_conflict", format!("mind already exists: {dest}::{new_name}")));
                }
                target_mode = cfg
                    .content_layout
                    .ok_or_else(|| reject("ownership_unresolved", "destination has no explicit layout"))?;
                touched.insert(dest.clone());
                if !configs.contains_key(dest) {
                    configs.insert(dest.clone(), before_config(&before, dest)?);
                }
                let item = registration(configs.get(&project_id).unwrap(), &owner);
                register(configs.get_mut(dest).unwrap(), &new_name, item);
            }
            remove_registration(configs.get_mut(&project_id).unwrap(), &owner.name);
            identities.insert(owner.identity.clone(), dest_project.as_ref().map(|p| format!("{p}::{new_name}")));
            if mode == ContentLayout::Bundle && (target_mode == mode || dest_project.is_none()) {
                let old = scoped(&project_id, &owner.name);
                if before.contains_key(&old) {
                    moves.insert(old, dest_project.as_ref().map(|p| scoped(p, &new_name)));
                }
            } else {
                for item in owned.iter().filter(|r| r.mind == owner.name) {
                    moves.insert(
                        scoped(&project_id, &item.path),
                        dest_project
                            .as_ref()
                            .map(|p| scoped(p, &inventory::destination(target_mode, &new_name, &item.kind, item.file))),
                    );
                }
                if mode == ContentLayout::Bundle {
                    let container = scoped(&project_id, &owner.name);
                    if before.keys().any(|p| {
                        p.starts_with(&format!("{container}/")) && !moves.keys().any(|m| Path::new(p).starts_with(m))
                    }) {
                        return Err(reject(
                            "ownership_unresolved",
                            "bundle contains hidden container files without a category role; move cannot leave owned files behind",
                        ));
                    }
                    cleanup.insert(container);
                }
            }
            if let Some(dest) = dest_project
                && target_mode == ContentLayout::Bundle
                && mode != ContentLayout::Bundle
            {
                let path = scoped(&dest, &new_name);
                if before.contains_key(&path) {
                    return Err(reject("path_conflict", path));
                }
                after.insert(path, Entry::Directory { mode: 0o755 });
            }
        }
        Operation::OutputRename { identity, .. }
        | Operation::OutputMove { identity, .. }
        | Operation::OutputRemove { identity } => {
            let output = mind::resolve_output(root, project, identity)?;
            let mut dest = project_id.clone();
            let mut owner = output.mind.clone();
            let mut name = output.name.clone();
            let mut dest_mode = mode;
            let removing = matches!(operation, Operation::OutputRemove { .. });
            match &operation {
                Operation::OutputRename { name: n, .. } => {
                    mind::validate_name(n)?;
                    name = n.clone();
                }
                Operation::OutputMove { mind: target_mind, project: target_project, .. } => {
                    let target = util::resolve_project(root, Some(target_project), root)?;
                    dest = util::rel_posix_path(root, &target)?;
                    owner = mind::resolve_mind(root, &target, target_mind)?.name;
                    dest_mode = mind::config_for(&target)?.content_layout.unwrap();
                    touched.insert(dest.clone());
                }
                _ => {}
            }
            let new_path = format!(
                "{}/{name}{}",
                mind::output_root(dest_mode, &owner),
                if output.shape == "file" { ".md" } else { "" }
            );
            if !removing {
                let target_owner = mind::resolve_mind(root, &root.join(&dest), &owner)?;
                if target_owner.outputs.iter().any(|o| o.name.eq_ignore_ascii_case(&name)) {
                    return Err(reject("path_conflict", format!("output exists: {dest}::{owner}::{name}")));
                }
            }
            moves.insert(scoped(&project_id, &output.path), (!removing).then(|| scoped(&dest, &new_path)));
            identities.insert(output.identity.clone(), (!removing).then(|| format!("{dest}::{owner}::{name}")));
            let cfg = configs.get_mut(&project_id).unwrap();
            if cfg["minds"][&output.mind]["default_output"].as_str() == Some(&output.name) {
                cfg["minds"][&output.mind]["default_output"] =
                    if !removing && dest == project_id && owner == output.mind { name.into() } else { Value::Null };
            }
        }
    }
    relocate(&before, &mut after, &moves)?;
    // Remove only explicit bundle containers that became empty. User placeholders stay.
    for path in cleanup {
        if !after.keys().any(|p| p.starts_with(&format!("{path}/"))) {
            after.remove(&path);
        }
    }
    for (id, cfg) in configs {
        put(&mut after, &scoped(&id, "mind.yaml"), &cfg)?;
    }
    rewrite_authored(&before, &mut after, &moves)?;
    metadata(root, &before, &mut after, &moves, &identities, &touched)?;
    ensure_parents(&mut after)?;
    Plan::new(root, op_name, watches, before, after)
}

// A previous bundle→category conversion may retain hidden container
// placeholders. Reusing that directory must not absorb unknown visible data.
fn placeholder_container(files: &Snapshot, path: &str) -> bool {
    matches!(files.get(path), Some(Entry::Directory { .. }))
        && files
            .keys()
            .filter_map(|p| p.strip_prefix(&format!("{path}/")))
            .all(|p| p.split('/').next().is_some_and(|first| first.starts_with('.')))
}

fn before_config(before: &Snapshot, project: &str) -> Result<Value> {
    raw(before, &scoped(project, "mind.yaml"))
}

fn relocate(before: &Snapshot, after: &mut Snapshot, moves: &Moves) -> Result<()> {
    let mut additions = BTreeMap::new();
    for (path, entry) in before {
        let target = references::relocated(path, moves);
        if target.as_deref() == Some(path) {
            continue;
        }
        after.remove(path);
        if let Some(target) = target
            && (before.contains_key(&target) || additions.insert(target.clone(), entry.clone()).is_some())
        {
            return Err(reject("path_conflict", target));
        }
    }
    after.extend(additions);
    Ok(())
}
fn ensure_parents(after: &mut Snapshot) -> Result<()> {
    let paths: Vec<_> = after.keys().cloned().collect();
    for path in paths {
        let mut parent = Path::new(&path).parent();
        while let Some(p) = parent.filter(|p| !p.as_os_str().is_empty()) {
            let p = p.to_string_lossy().into_owned();
            match after.get(&p) {
                Some(Entry::File { .. }) => return Err(reject("path_conflict", p)),
                Some(_) => {}
                None => {
                    after.insert(p, Entry::Directory { mode: 0o755 });
                }
            }
            parent = parent.and_then(Path::parent);
        }
    }
    Ok(())
}
fn rewrite_authored(before: &Snapshot, after: &mut Snapshot, moves: &Moves) -> Result<()> {
    let mut errors = Vec::new();
    for (path, entry) in before {
        if !matches!(Path::new(path).extension().and_then(|s| s.to_str()), Some("md" | "html" | "htm")) {
            continue;
        }
        let Some(new_path) = references::relocated(path, moves) else { continue };
        let Entry::File { bytes, mode } = entry else { continue };
        let text = std::str::from_utf8(bytes)
            .map_err(|_| reject("reference_unsupported", format!("non UTF-8 authored file: {path}")))?;
        match references::rewrite(text, path, &new_path, moves) {
            Ok(text) => {
                after.insert(new_path, Entry::File { bytes: text.into_bytes(), mode: *mode });
            }
            Err(e) => errors.push(e),
        }
    }
    if let Some(first) = errors.first() {
        let code = if errors.iter().any(|e| e.kind() == "inbound_reference") {
            "inbound_reference"
        } else {
            "reference_unsupported"
        };
        return Err(reject(
            code,
            format!("{}\n{}", first, errors.iter().skip(1).map(ToString::to_string).collect::<Vec<_>>().join("\n")),
        ));
    }
    Ok(())
}

fn project_of(path: &str, projects: &[String]) -> Option<String> {
    projects.iter().filter(|p| path.starts_with(&prefix(p))).max_by_key(|p| p.len()).cloned()
}

fn metadata(
    root: &Path,
    before: &Snapshot,
    after: &mut Snapshot,
    moves: &Moves,
    identities: &BTreeMap<String, Option<String>>,
    touched: &BTreeSet<String>,
) -> Result<()> {
    let projects =
        mind::project_paths(root)?.iter().map(|p| util::rel_posix_path(root, p)).collect::<Result<Vec<_>>>()?;
    let mut touched = touched.clone();
    for (path, entry) in after.iter() {
        if before.get(path) != Some(entry)
            && let Some(project) = project_of(path, &projects)
        {
            touched.insert(project);
        }
    }
    let mut indexes = BTreeMap::new();
    for project in &projects {
        indexes.insert(project.clone(), raw(before, &scoped(project, "mind-index.yaml"))?);
    }
    // Transfer authority rows using their explicit paths, preserving all other
    // fields. Unknown entries without a path cannot be assigned from a title.
    for section in ["sources", "assets", "publish_records", "prompts", "thinking", "articles"] {
        let mut rows: BTreeMap<String, Vec<(Option<Value>, Value)>> = BTreeMap::new();
        for project in &projects {
            let value = indexes[project][section].clone();
            let entries: Vec<_> = match value {
                Value::Sequence(seq) => seq.into_iter().map(|v| (None, v)).collect(),
                Value::Mapping(map) => map.into_iter().map(|(k, v)| (Some(k), v)).collect(),
                Value::Null => vec![],
                _ => return Err(reject("invalid_metadata", format!("{project}: {section}"))),
            };
            for (mut key, mut row) in entries {
                let field = if section == "articles" { "article_path" } else { "path" };
                let path =
                    row[field].as_str().or_else(|| key.as_ref().and_then(Value::as_str).filter(|s| s.contains('/')));
                let Some(path) = path.map(str::to_owned) else {
                    if touched.contains(project) && matches!(section, "sources" | "assets" | "articles") {
                        return Err(reject(
                            "ownership_unresolved",
                            format!("{project}: {section} registration lacks explicit path"),
                        ));
                    }
                    rows.entry(project.clone()).or_default().push((key, row));
                    continue;
                };
                let full = scoped(project, path.trim_start_matches("./"));
                if touched.contains(project)
                    && matches!(section, "sources" | "assets")
                    && (!before.contains_key(&full)
                        || Path::new(&path)
                            .components()
                            .any(|c| matches!(c, std::path::Component::ParentDir | std::path::Component::RootDir)))
                {
                    return Err(reject(
                        "ownership_unresolved",
                        format!("{project}: {section} registration has an absent or external path: {path}"),
                    ));
                }
                let Some(mapped) = references::relocated(&full, moves) else { continue };
                let target = project_of(&mapped, &projects)
                    .ok_or_else(|| reject("invalid_metadata", "registration moved outside a known project"))?;
                if mapped != full {
                    let rel = mapped.strip_prefix(&prefix(&target)).unwrap();
                    row[field] = rel.into();
                    if key.as_ref().and_then(Value::as_str) == Some(&path) {
                        key = Some(rel.into());
                    }
                    if row.get("project").is_some() {
                        row["project"] = target.clone().into();
                    }
                }
                rows.entry(target).or_default().push((key, row));
            }
        }
        for project in &projects {
            let entries = rows.remove(project).unwrap_or_default();
            if entries.is_empty() && indexes[project].get(section).is_none() {
                continue;
            }
            let original = &indexes[project][section];
            let use_map = original.is_mapping() || (original.is_null() && entries.iter().all(|(key, _)| key.is_some()));
            let value = if use_map {
                let mut map = serde_yaml::Mapping::new();
                for (key, row) in entries {
                    let key = key
                        .or_else(|| row.get("name").cloned())
                        .ok_or_else(|| reject("invalid_metadata", "cannot determine registration key"))?;
                    if map.insert(key, row).is_some() {
                        return Err(reject("path_conflict", format!("{project}: duplicate {section} key")));
                    }
                }
                Value::Mapping(map)
            } else {
                Value::Sequence(entries.into_iter().map(|(_, row)| row).collect())
            };
            indexes.get_mut(project).unwrap()[section] = value;
        }
    }
    let mut outputs: BTreeMap<String, Vec<crate::model::mind::Output>> = BTreeMap::new();
    for project in &projects {
        if mind::config_for(&root.join(project))?.content_layout.is_none() {
            continue;
        }
        for output in mind::discover(root, &root.join(project))?.into_iter().flat_map(|m| m.outputs) {
            let Some(identity) = identity_map(&output.identity, identities) else { continue };
            let Some(path) = references::relocated(&scoped(project, &output.path), moves) else { continue };
            let parts: Vec<_> = identity.split("::").collect();
            if parts.len() != 3 {
                return Err(reject("invalid_metadata", identity));
            }
            let mut item = output;
            item.project_identity = parts[0].into();
            item.mind = parts[1].into();
            item.name = parts[2].into();
            item.mind_identity = format!("{}::{}", parts[0], parts[1]);
            item.path = path.strip_prefix(&prefix(parts[0])).ok_or_else(|| reject("invalid_metadata", &path))?.into();
            item.build_path = format!("build/{}/{}.md", parts[1], parts[2]);
            item.identity = identity.clone();
            item.output_identity = identity;
            outputs.entry(item.project_identity.clone()).or_default().push(item);
        }
    }
    for project in &projects {
        let index = indexes.get_mut(project).unwrap();
        if touched.contains(project) {
            index["outputs"] = serde_yaml::to_value(outputs.remove(project).unwrap_or_default()).map_err(yaml_error)?;
            let cfg: crate::model::config::MindConfig =
                serde_yaml::from_value(raw(after, &scoped(project, "mind.yaml"))?).map_err(yaml_error)?;
            let mode = cfg.content_layout;
            if let Some(mode) = mode {
                let mut extensions = Vec::new();
                for source in &projects {
                    if mind::config_for(&root.join(source))?.content_layout.is_none() {
                        continue;
                    }
                    for item in crate::service::additional::discover(root, &root.join(source))? {
                        let old_owner = item.mind_identity.clone();
                        let Some(owner) = identity_map(&old_owner, identities) else { continue };
                        let Some(path) = references::relocated(&scoped(source, &item.path), moves) else { continue };
                        if project_of(&path, &projects).as_ref() != Some(project) {
                            continue;
                        }
                        let rel = path.strip_prefix(&prefix(project)).unwrap();
                        let parts: Vec<_> = rel.split('/').collect();
                        let kind = if mode == ContentLayout::Category {
                            parts[0]
                        } else {
                            parts[1].strip_suffix(".md").unwrap_or(parts[1])
                        };
                        let owner_name = owner.rsplit("::").next().unwrap();
                        extensions.push(crate::service::additional::from_owned(
                            project,
                            &inventory::OwnedRoot {
                                mind: owner_name.into(),
                                kind: kind.into(),
                                path: rel.into(),
                                file: item.shape == "file",
                            },
                        ));
                    }
                }
                extensions.sort_by(|a, b| a.identity.cmp(&b.identity));
                index["additional"] = serde_yaml::to_value(extensions).map_err(yaml_error)?;
            }
            index["rag_stale"] = true.into();
        }
        let path = scoped(project, "mind-index.yaml");
        if index != &raw(before, &path)? {
            put(after, &path, index)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    fn fixture() -> (tempfile::TempDir, Plan) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("minds.yaml"), "schema: '1'\nprojects: [p]\n").unwrap();
        for path in ["p/outputs/a", "p/assets/a", "p/reviews", "p/sources/a"] {
            fs::create_dir_all(root.join(path)).unwrap();
        }
        fs::write(root.join("p/mind.yaml"), "content_layout: category\nminds:\n  a: {}\n  zero: {}\n").unwrap();
        fs::write(root.join("p/outputs/a/one.md"), "![x](../../assets/a/x.png)\n").unwrap();
        fs::write(root.join("p/outputs/a/two.md"), "# Two\n").unwrap();
        fs::write(root.join("p/assets/a/x.png"), [0, 255, 3]).unwrap();
        fs::write(root.join("p/sources/a/e.md"), "# Evidence\n").unwrap();
        fs::write(root.join("p/reviews/a.md"), "# Review\n").unwrap();
        fs::write(root.join("p/mind-index.yaml"), "sources:\n  e:\n    path: sources/a/e.md\nassets:\n  x:\n    path: assets/a/x.png\npublish_records:\n  p:\n    path: outputs/a/one.md\n    target_name: local\n").unwrap();
        let planned = plan(root, &root.join("p"), Operation::Convert(ContentLayout::Bundle)).unwrap();
        (dir, planned)
    }
    #[test]
    fn full_conversion_every_durable_stage_can_resume_or_rollback() {
        let (_, sample) = fixture();
        for rollback in [false, true] {
            for stage in 0..=sample.preview().len() {
                let (dir, plan) = fixture();
                assert_eq!(
                    transaction::apply(dir.path(), &plan, Some(stage)).unwrap_err().kind(),
                    "injected_failure",
                    "{rollback} stage {stage}"
                );
                transaction::recover(dir.path(), rollback, None).unwrap();
                assert_eq!(
                    transaction::snapshot(dir.path(), &plan.watches).unwrap(),
                    if rollback { plan.before.clone() } else { plan.after.clone() },
                    "{rollback} stage {stage}"
                );
                transaction::recover(dir.path(), rollback, None).unwrap();
            }
        }
    }
    #[test]
    fn root_conversion_every_durable_stage_can_resume_or_rollback() {
        for rollback in [false, true] {
            let mut stage = 0;
            loop {
                let (dir, _) = fixture();
                let root = dir.path();
                for entry in fs::read_dir(root.join("p")).unwrap() {
                    let entry = entry.unwrap();
                    fs::rename(entry.path(), root.join(entry.file_name())).unwrap();
                }
                fs::remove_dir(root.join("p")).unwrap();
                fs::write(root.join("minds.yaml"), "schema: '1'\nprojects: ['.']\n").unwrap();
                let plan = plan(root, root, Operation::Convert(ContentLayout::Bundle)).unwrap();
                if stage > plan.preview().len() {
                    break;
                }
                assert_eq!(transaction::apply(root, &plan, Some(stage)).unwrap_err().kind(), "injected_failure");
                transaction::recover(root, rollback, None).unwrap();
                assert_eq!(
                    transaction::snapshot(root, &plan.watches).unwrap(),
                    if rollback { plan.before } else { plan.after }
                );
                transaction::recover(root, rollback, None).unwrap();
                stage += 1;
            }
        }
    }
    #[test]
    fn actual_lance_tables_are_refused_without_touching_authority() {
        use crate::service::source::advanced::{advanced_store_dir, lance_store::LanceStore};
        let (dir, plan) = fixture();
        let store_path = advanced_store_dir(dir.path());
        let store = LanceStore::create(&store_path).unwrap();
        store.ensure_tables().unwrap();
        let tables = store.rt().block_on(store.db().table_names().execute()).unwrap();
        assert!(!tables.is_empty());
        assert_eq!(transaction::apply(dir.path(), &plan, None).unwrap_err().kind(), "source_backend_unsupported");
        assert_eq!(store.rt().block_on(store.db().table_names().execute()).unwrap(), tables);
        assert_eq!(transaction::snapshot(dir.path(), &plan.watches).unwrap(), plan.before);
        assert!(!dir.path().join(".mind-forge/lifecycle").exists());
    }
}
