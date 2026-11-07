//! Deterministic disk/projection and explicit binding comparison. Never infer
//! ownership from prose, titles, feedback status, or imported_by.
use crate::error::{MfError, Result};
use crate::service::{additional, mind};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};

pub fn inspect(root: &Path, project: &Path) -> Result<Value> {
    let minds = mind::discover(root, project)?;
    let outputs: Vec<_> = minds.iter().flat_map(|m| m.outputs.clone()).collect();
    let extensions = additional::discover(root, project)?;
    let path = project.join("mind-index.yaml");
    let index: Value = if path.exists() {
        serde_yaml::from_str(&std::fs::read_to_string(&path)?).map_err(|e| MfError::ParseError {
            kind: "yaml".into(),
            path: path.clone(),
            detail: e.to_string(),
        })?
    } else {
        json!({})
    };
    let mut issues = Vec::new();
    for (section, disk) in
        [("outputs", serde_json::to_value(&outputs)?), ("additional", serde_json::to_value(&extensions)?)]
    {
        let expected: BTreeMap<_, _> =
            disk.as_array().unwrap().iter().map(|v| (v["identity"].as_str().unwrap().to_string(), v)).collect();
        let rows = match index.get(section) {
            None | Some(Value::Null) => vec![],
            Some(Value::Array(rows)) => rows.iter().collect(),
            _ => {
                issues.push(json!({"kind":"invalid_projection", "section":section, "candidates":disk}));
                continue;
            }
        };
        let mut seen = BTreeMap::new();
        for row in rows {
            let identity = row["identity"].as_str().unwrap_or("");
            let key = identity.to_string();
            if seen.insert(key.clone(), row).is_some() {
                issues.push(json!({"kind":"duplicate_projection", "section":section, "identity":identity, "indexed":row, "candidates":expected.get(&key)}));
            }
            match expected.get(&key) {
                None => issues.push(json!({"kind":"stale_projection", "section":section, "identity":identity, "indexed":row, "candidates":disk})),
                Some(actual) if *actual != row => issues.push(json!({"kind":"projection_mismatch", "section":section, "identity":identity, "indexed":row, "candidates":[actual]})),
                _ => {}
            }
        }
        for (identity, actual) in expected {
            if !seen.contains_key(&identity) {
                issues.push(
                    json!({"kind":"missing_projection", "section":section, "identity":identity, "candidates":[actual]}),
                );
            }
        }
    }
    for owner in &minds {
        if let Some(default) = &owner.default_output
            && !owner.outputs.iter().any(|o| &o.name == default)
        {
            issues.push(json!({"kind":"missing_default_output", "identity":owner.identity, "default_output":default, "candidates":owner.outputs.iter().map(|o|&o.identity).collect::<Vec<_>>()}));
        }
    }
    // Authority rows stay intact; report absent/unsafe paths, duplicate bindings,
    // or an explicit owner that disagrees with structural ownership.
    for section in ["sources", "assets", "prompts", "thinking", "publish_records"] {
        let rows: Vec<_> = match index.get(section) {
            None | Some(Value::Null) => vec![],
            Some(Value::Array(rows)) => rows.iter().map(|v| ("", v)).collect(),
            Some(Value::Object(rows)) => rows.iter().map(|(k, v)| (k.as_str(), v)).collect(),
            _ => {
                issues.push(json!({"kind":"invalid_binding", "section":section, "candidates":[]}));
                continue;
            }
        };
        let mut bindings: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (key, row) in rows {
            let path = row["path"].as_str().or_else(|| key.contains('/').then_some(key));
            let Some(path) = path else {
                issues.push(json!({"kind":"binding_path_missing", "section":section, "key":key, "candidates":[]}));
                continue;
            };
            let relative = path.strip_prefix("./").unwrap_or(path);
            let safe = mind::checked_path(project, relative);
            if safe.as_ref().is_err() || !safe.as_ref().is_ok_and(|p| p.exists()) {
                issues.push(json!({"kind":if safe.is_err(){"unsafe_binding"}else{"missing_binding_target"}, "section":section,"key":key,"path":path,"candidates":[]}));
                continue;
            }
            let owners: Vec<_> = minds
                .iter()
                .filter(|m| {
                    let role = if section == "prompts" { "prompt" } else { section };
                    if section == "publish_records" {
                        m.outputs.iter().any(|o| Path::new(relative).starts_with(&o.path))
                    } else {
                        m.roles.get(role).is_some_and(|p| Path::new(relative).starts_with(p))
                    }
                })
                .collect();
            let candidates: Vec<_> = owners.iter().map(|m| &m.identity).collect();
            let claimed = row["mind_identity"].as_str().or_else(|| row["mind"].as_str());
            if owners.len() != 1 || claimed.is_some_and(|c| c != owners[0].identity && c != owners[0].name) {
                issues.push(json!({"kind":"binding_mismatch", "section":section, "key":key,"path":path,"claimed":claimed,"candidates":candidates}));
            }
            bindings.entry(relative.into()).or_default().push(key.into());
        }
        for (path, keys) in bindings {
            if keys.len() > 1 {
                issues.push(json!({"kind":"duplicate_binding", "section":section,"path":path,"candidates":keys}));
            }
        }
    }
    issues.sort_by_key(Value::to_string);
    Ok(
        json!({"kind":"content_check", "project_identity":crate::service::util::rel_posix_path(root,project)?, "consistent":issues.is_empty(), "issues":issues, "disk":{"outputs":outputs,"additional":extensions}}),
    )
}

pub fn reconcile(root: &Path, project: &Path, dry_run: bool) -> Result<Value> {
    let report = inspect(root, project)?;
    if !dry_run {
        let mut index = crate::service::index::load(project)?;
        let extra = index.extra.get_or_insert_with(Vec::new);
        for section in ["outputs", "additional"] {
            extra.retain(|(key, _)| key != section);
            extra.push((section.into(), report["disk"][section].clone()));
        }
        crate::service::index::save(project, &index)?;
    }
    Ok(
        json!({"kind":"content_index", "outputs":report["disk"]["outputs"], "additional":report["disk"]["additional"], "differences":report["issues"], "dry_run":dry_run}),
    )
}
