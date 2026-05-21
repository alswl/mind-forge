//! Remove a term from project scope or the global pool.

use std::path::Path;

use crate::error::{MfError, Result};
use crate::model::lifecycle::PlannedChange;
use crate::model::term::{TermIdentity, TermRemoveReport};
use crate::service::index;
use crate::service::lifecycle;

/// Remove a project-scoped term from `mind-index.yaml`.
pub fn remove_term(project_path: &Path, term_name: &str, force: bool, dry_run: bool) -> Result<TermRemoveReport> {
    crate::service::util::require_nonempty(term_name, "term name")?;

    let mut index = index::load(project_path)?;

    // Check term exists
    let terms = index.terms.as_ref().ok_or_else(|| {
        MfError::usage(
            format!("term '{term_name}' not found"),
            Some("use 'mf term list -p <project>' to see available terms".to_string()),
        )
    })?;

    if !terms.iter().any(|t| t.term == term_name) {
        return Err(MfError::usage(
            format!("term '{term_name}' not found"),
            Some("use 'mf term list -p <project>' to see available terms".to_string()),
        ));
    }

    let scope = lifecycle::resolve_scope(Some(project_path), false)?;
    let before = TermIdentity { name: term_name.to_string(), scope };

    // Reference scan (before mutating)
    let refs = lifecycle::scan_references(project_path, &index, crate::model::lifecycle::ObjectKind::Term, term_name);

    if !refs.is_empty() && !force {
        let ref_ids: Vec<&str> = refs.iter().map(|r| r.from_id.as_str()).collect();
        return Err(MfError::usage(
            format!("term '{term_name}' is referenced by: {}. Use --force to remove anyway.", ref_ids.join(", ")),
            Some("check which articles reference this term before removal".to_string()),
        ));
    }

    let planned: Vec<PlannedChange> = vec![
        lifecycle::planned_yaml_update(&project_path.join("mind-index.yaml").to_string_lossy(), Some(term_name), None),
        lifecycle::planned_index_refresh(&project_path.join("mind-index.yaml").to_string_lossy()),
    ];

    if dry_run {
        return Ok(TermRemoveReport {
            verb: "remove".into(),
            kind: "term".into(),
            before,
            after: None,
            references: refs,
            side_effects: planned,
            force,
            dry_run: true,
        });
    }

    // Now mutate
    {
        let terms = index.terms.as_mut().expect("already checked");
        let pos = terms.iter().position(|t| t.term == term_name).expect("already checked");
        terms.remove(pos);
    }
    index::save(project_path, &index)?;

    Ok(TermRemoveReport {
        verb: "remove".into(),
        kind: "term".into(),
        before,
        after: None,
        references: refs,
        side_effects: planned,
        force,
        dry_run: false,
    })
}

/// Remove a term from the global `minds-terms.yaml` pool.
pub fn remove_term_global(repo_root: &Path, term_name: &str, force: bool, dry_run: bool) -> Result<TermRemoveReport> {
    crate::service::util::require_nonempty(term_name, "term name")?;

    let mut terms = crate::service::term::repo_format::load(repo_root)?;

    if !terms.iter().any(|t| t.term == term_name) {
        return Err(MfError::usage(
            format!("term '{term_name}' not found in global terms"),
            Some("use 'mf term list --global' to see available terms".to_string()),
        ));
    }

    let scope = lifecycle::resolve_scope(None, true)?;
    let before = TermIdentity { name: term_name.to_string(), scope };

    // For global terms, reference scanning is minimal (no per-project articles)
    let refs: Vec<crate::model::lifecycle::Reference> = Vec::new();

    let global_terms_path = repo_root.join("minds-terms.yaml");
    let planned: Vec<PlannedChange> =
        vec![lifecycle::planned_yaml_update(&global_terms_path.to_string_lossy(), Some(term_name), None)];

    if dry_run {
        return Ok(TermRemoveReport {
            verb: "remove".into(),
            kind: "term".into(),
            before,
            after: None,
            references: refs,
            side_effects: planned,
            force,
            dry_run: true,
        });
    }

    terms.retain(|t| t.term != term_name);
    crate::service::term::global::save_terms(repo_root, &terms)?;

    Ok(TermRemoveReport {
        verb: "remove".into(),
        kind: "term".into(),
        before,
        after: None,
        references: refs,
        side_effects: planned,
        force,
        dry_run: false,
    })
}
