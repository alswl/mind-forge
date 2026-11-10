//! Structural extension resources; no registry of allowed categories.
use crate::error::{MfError, Result};
use crate::service::{
    mind,
    mind_lifecycle::inventory::{self, OwnedRoot},
    util,
};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Additional {
    pub kind: String,
    pub identity: String,
    pub project_identity: String,
    pub mind_identity: String,
    pub mind: String,
    pub additional_kind: String,
    pub path: String,
    pub shape: String,
}

pub fn is_additional(item: &OwnedRoot) -> bool {
    !matches!(item.kind.as_str(), "prompt" | "thinking" | "outputs" | "sources" | "assets")
}

pub fn from_owned(project_identity: &str, item: &OwnedRoot) -> Additional {
    let mind_identity = format!("{project_identity}::{}", item.mind);
    Additional {
        kind: "additional".into(),
        identity: format!("{mind_identity}::additional::{}", item.kind),
        project_identity: project_identity.into(),
        mind_identity,
        mind: item.mind.clone(),
        additional_kind: item.kind.clone(),
        path: item.path.clone(),
        shape: if item.file { "file" } else { "directory" }.into(),
    }
}

pub fn discover(root: &Path, project: &Path) -> Result<Vec<Additional>> {
    let project_id = util::rel_posix_path(root, project)?;
    let mut items: Vec<_> = inventory::scan(root, project)?
        .iter()
        .filter(|item| is_additional(item))
        .map(|item| from_owned(&project_id, item))
        .collect();
    items.sort_by(|a, b| a.identity.cmp(&b.identity));
    Ok(items)
}

pub fn select(items: &[Additional], selector: &str) -> Result<Additional> {
    if selector.contains('/') && !selector.contains("::") {
        // Reject traversal even when no matching resource exists.
        if Path::new(selector).components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
            return Err(MfError::Lifecycle { code: "unsafe_path", message: selector.into() });
        }
    }
    if let Some(item) = items.iter().find(|i| i.identity == selector || i.path == selector) {
        return Ok(item.clone());
    }
    let candidates: Vec<_> = items
        .iter()
        .filter(|i| {
            i.additional_kind == selector || format!("{}::additional::{}", i.mind, i.additional_kind) == selector
        })
        .collect();
    match candidates.as_slice() {
        [item] => Ok((*item).clone()),
        [] => Err(MfError::not_found(format!("additional not found: {selector}"), None)),
        _ => Err(MfError::SelectionAmbiguous {
            selector: selector.into(),
            candidates: candidates.iter().map(|i| i.identity.clone()).collect(),
        }),
    }
}

/// Renaming one resource retains its owner and shape, including in category
/// layout where the filename encodes ownership. The destination is explicit.
pub fn rename_path(
    root: &Path,
    project: &Path,
    item: &Additional,
    target: &str,
    items: &[Additional],
) -> Result<String> {
    mind::checked_path(project, target)?;
    let cfg = mind::config_for(project)?;
    let parts: Vec<_> = target.split('/').collect();
    let file = item.shape == "file";
    let invalid = || MfError::Lifecycle {
        code: "invalid_additional_target",
        message: "target must be a concrete additional path with the same mind and shape".into(),
    };
    if parts.len() != 2 {
        return Err(invalid());
    }
    let kind = match cfg.content_layout.unwrap() {
        crate::model::mind::ContentLayout::Category => {
            if parts[1] != format!("{}{}", item.mind, if file { ".md" } else { "" }) {
                return Err(invalid());
            }
            parts[0]
        }
        crate::model::mind::ContentLayout::Bundle => {
            if parts[0] != item.mind {
                return Err(invalid());
            }
            if file { parts[1].strip_suffix(".md").ok_or_else(invalid)? } else { parts[1] }
        }
    };
    let mut target_cfg = cfg.clone();
    if cfg.content_layout == Some(crate::model::mind::ContentLayout::Bundle) {
        target_cfg.reserved_roots.clear();
    }
    let reserved = mind::reserved(&target_cfg);
    if !inventory::valid_kind(kind)
        || kind == "prompt"
        || reserved.iter().any(|r| r.eq_ignore_ascii_case(kind))
        || mind::child_project_roots(root, project)?.contains(kind)
    {
        return Err(invalid());
    }
    let collisions: Vec<_> = items
        .iter()
        .filter(|i| i.mind == item.mind && i.additional_kind.eq_ignore_ascii_case(kind))
        .map(|i| i.path.clone())
        .collect();
    if !collisions.is_empty() {
        return Err(MfError::ContentConflict { code: "path_conflict", message: target.into(), candidates: collisions });
    }
    // Parent case collisions matter on case-insensitive and case-sensitive hosts.
    let parent = project.join(parts[0]);
    for entry in std::fs::read_dir(project)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.eq_ignore_ascii_case(parts[0]) && (name != parts[0] || !entry.file_type()?.is_dir()) {
            return Err(MfError::ContentConflict {
                code: "path_conflict",
                message: target.into(),
                candidates: vec![name],
            });
        }
    }
    if parent.join(parts[1]).exists() {
        return Err(MfError::file_exists(parent.join(parts[1])));
    }
    Ok(kind.into())
}

#[cfg(test)]
mod tests {
    use crate::service::mind_lifecycle::{
        planner::{self, Operation},
        transaction,
    };
    use std::fs;

    #[test]
    fn additional_rename_and_remove_recover_at_every_durable_stage() {
        for mode in ["category", "bundle"] {
            for remove in [false, true] {
                for rollback in [false, true] {
                    let mut stage = 0;
                    loop {
                        let dir = tempfile::tempdir().unwrap();
                        let root = dir.path();
                        fs::write(root.join("minds.yaml"), "schema: '1'\nprojects: ['.']\n").unwrap();
                        fs::write(root.join("mind.yaml"), format!("content_layout: {mode}\nminds:\n  a: {{}}\n"))
                            .unwrap();
                        let old = if mode == "category" { "evals/a" } else { "a/evals" };
                        let new = if mode == "category" { "results/a" } else { "a/results" };
                        fs::create_dir_all(root.join(old)).unwrap();
                        fs::write(root.join(old).join("data.json"), "{\"evidence\":true}\n").unwrap();
                        let op = if remove {
                            Operation::AdditionalRemove { identity: old.into() }
                        } else {
                            Operation::AdditionalRename { identity: old.into(), destination: new.into() }
                        };
                        let plan = planner::plan(root, root, op).unwrap();
                        if stage > plan.preview().len() {
                            break;
                        }
                        assert_eq!(
                            transaction::apply(root, &plan, Some(stage)).unwrap_err().kind(),
                            "injected_failure"
                        );
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
        }
    }
}
