use std::path::Path;

use serde::Serialize;

use crate::error::{MfError, Result};
use crate::model::lifecycle::PlannedChange;
use crate::service::{lifecycle, util};

/// Report from a successful term rename.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct TermRenameReport {
    pub verb: String,
    pub kind: String,
    pub before: TermRenameIdentity,
    pub after: TermRenameIdentity,
    #[serde(default)]
    pub references: Vec<crate::model::lifecycle::Reference>,
    #[serde(default)]
    pub side_effects: Vec<PlannedChange>,
    pub force: bool,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct TermRenameIdentity {
    pub name: String,
    pub scope: crate::model::lifecycle::ScopeRef,
}

/// Rename a project-scoped term.
pub fn rename_term(
    project_path: &Path,
    old_name: &str,
    new_name: &str,
    keep_alias: bool,
    force: bool,
    dry_run: bool,
) -> Result<TermRenameReport> {
    util::require_nonempty(old_name, "old term name")?;
    util::require_nonempty(new_name, "new term name")?;

    let mut index = crate::service::index::load(project_path)?;
    let terms = index.terms.as_ref().ok_or_else(|| {
        MfError::not_found(
            format!("term '{old_name}' not found"),
            Some("use `mf term list` to see available terms".to_string()),
        )
    })?;

    let pos = terms.iter().position(|t| t.term == old_name).ok_or_else(|| {
        MfError::not_found(
            format!("term '{old_name}' not found"),
            Some("use `mf term list` to see available terms".to_string()),
        )
    })?;

    // Check for duplicate new_name
    if old_name != new_name && terms.iter().any(|t| t.term == new_name) && !force {
        return Err(MfError::usage(
            format!("a term named '{new_name}' already exists"),
            Some("use --force to overwrite".to_string()),
        ));
    }

    let scope = lifecycle::resolve_scope(Some(project_path), false)?;
    let before = TermRenameIdentity { name: old_name.to_string(), scope: scope.clone() };
    let after = TermRenameIdentity { name: new_name.to_string(), scope };

    let planned: Vec<PlannedChange> = vec![lifecycle::planned_yaml_update(
        &project_path.join("mind-index.yaml").to_string_lossy(),
        Some(old_name),
        Some(new_name),
    )];

    if dry_run {
        return Ok(TermRenameReport {
            verb: "rename".into(),
            kind: "term".into(),
            before,
            after,
            references: vec![],
            side_effects: planned,
            force,
            dry_run: true,
        });
    }

    let terms = index.terms.as_mut().unwrap();
    let term = &mut terms[pos];

    if keep_alias && !term.aliases.contains(&old_name.to_string()) {
        term.aliases.push(old_name.to_string());
    }
    term.term = new_name.to_string();

    crate::service::index::save(project_path, &index)?;

    Ok(TermRenameReport {
        verb: "rename".into(),
        kind: "term".into(),
        before,
        after,
        references: vec![],
        side_effects: planned,
        force,
        dry_run: false,
    })
}
