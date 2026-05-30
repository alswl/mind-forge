use std::path::Path;

use crate::error::{MfError, Result};
use crate::model::lifecycle::PlannedChange;
use crate::model::project::{ProjectIdentity, ProjectRemoveReport};
use crate::service::{repo, util};

/// Hard-remove a project: delete the project directory and update `minds.yaml`.
pub fn remove_project(repo_root: &Path, project_name: &str, force: bool, dry_run: bool) -> Result<ProjectRemoveReport> {
    util::require_nonempty(project_name, "project name")?;

    let projects_dir = repo::projects_dir_for(repo_root)?;
    let project_path = util::project_dir_for(repo_root, &projects_dir, project_name);

    if !project_path.exists() {
        return Err(MfError::usage(
            format!("project '{project_name}' not found at {}", project_path.display()),
            Some("use `mf project list` to see available projects".to_string()),
        ));
    }

    let before = ProjectIdentity { name: project_name.to_string(), path: project_path.to_string_lossy().to_string() };

    let planned: Vec<PlannedChange> = vec![
        PlannedChange {
            op: crate::model::lifecycle::PlannedOp::RemoveDir,
            path: project_path.to_string_lossy().to_string(),
            old: Some(project_name.to_string()),
            new: None,
        },
        PlannedChange {
            op: crate::model::lifecycle::PlannedOp::UpdateYaml,
            path: repo_root.join("minds.yaml").to_string_lossy().to_string(),
            old: Some(project_name.to_string()),
            new: None,
        },
    ];

    if dry_run {
        return Ok(ProjectRemoveReport {
            verb: "remove".into(),
            kind: "project".into(),
            before,
            after: None,
            references: vec![],
            side_effects: planned,
            force,
            dry_run: true,
        });
    }

    // Remove the project directory
    std::fs::remove_dir_all(&project_path).map_err(MfError::Io)?;

    // Update minds.yaml: remove the project entry
    let minds_path = repo_root.join("minds.yaml");
    if minds_path.exists() {
        let mut manifest = repo::load_manifest(&minds_path)?;
        manifest.projects.retain(|p| p.name != project_name);
        repo::save_manifest(&manifest, &minds_path)?;
    }

    Ok(ProjectRemoveReport {
        verb: "remove".into(),
        kind: "project".into(),
        before,
        after: None,
        references: vec![],
        side_effects: planned,
        force,
        dry_run: false,
    })
}
