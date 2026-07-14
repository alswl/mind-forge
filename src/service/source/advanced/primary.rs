//! Lance-primary Source fact mutations.
//!
//! These helpers deliberately update the registration catalog first and only
//! then export the legacy YAML compatibility projection.  They keep regular
//! Source commands from accidentally making a compatibility file authoritative
//! again after activation.

use std::path::Path;

use crate::error::{MfError, Result};
use crate::model::source::{FileKind, Source, SourceKind, SourceRemoveReport};
use crate::model::source_advanced::{RegistrationState, SourceRegistration};

use super::{catalog::SourceCatalog, config::ResolvedSourceConfig, identity, sync};

pub fn update_registration(
    repo_root: &Path,
    project_path: &Path,
    identity_name: &str,
    rename: Option<&str>,
    url: Option<&str>,
) -> Result<Source> {
    if rename.is_none() && url.is_none() {
        return Err(MfError::usage("nothing to update: use --rename or --url".to_string(), None));
    }
    if let Some(url) = url {
        crate::service::source::validate_url(url)?;
    }
    let config = ResolvedSourceConfig::from_config(
        crate::service::repo::load_manifest(&repo_root.join("minds.yaml"))?.source.as_ref(),
    )?;
    if !config.is_lance() {
        return Err(MfError::usage("Lance primary mutation requires an active Lance backend".to_string(), None));
    }
    let store = sync::open_active_store(repo_root)?;
    let catalog = SourceCatalog::discover(&config, repo_root)?;
    let project_rel = project_path.strip_prefix(repo_root).unwrap_or(project_path).to_string_lossy().replace('\\', "/");
    let rows = catalog.registrations(Some(&store))?;
    let current = rows
        .iter()
        .find(|row| row.project_path == project_rel && row.source_identity == identity_name)
        .ok_or_else(|| {
            MfError::usage(
                format!("source '{identity_name}' not found"),
                Some("use `mf source list` to see available sources".to_string()),
            )
        })?;
    let next_name = rename.unwrap_or(identity_name);
    if next_name != identity_name
        && rows.iter().any(|row| row.project_path == project_rel && row.source_identity == next_name)
    {
        return Err(MfError::usage(format!("a source named '{next_name}' already exists"), None));
    }
    let next_location = url.unwrap_or(&current.registered_location);
    let next_key = identity::registration_key(&current.project_key, &current.source_type, next_location);
    let registration = SourceRegistration {
        registration_key: next_key.clone(),
        project_key: current.project_key.clone(),
        project_identity: current.project_identity.clone(),
        project_path: current.project_path.clone(),
        source_identity: next_name.to_string(),
        source_type: current.source_type.clone(),
        source_kind: current.source_kind.clone(),
        registered_location: next_location.to_string(),
        tags_json: current.tags_json.clone(),
        fact_fingerprint: identity::raw_fingerprint(
            format!("{next_name}\n{}\n{next_location}\n{}", current.source_type, current.tags_json).as_bytes(),
        ),
        registration_revision: 1,
        state: RegistrationState::Live,
    };
    if next_key != current.registration_key {
        store.clear_content_bindings(&std::collections::BTreeSet::from([current.registration_key.clone()]))?;
    }
    store.delete_rows("registrations", &format!("registration_key = '{}'", current.registration_key))?;
    store.append_registrations(&[registration])?;
    super::compatibility::export_project(repo_root, &current.project_identity, false)?;

    let kind = match current.source_type.as_str() {
        "pdf" => FileKind::Pdf,
        "rss" => FileKind::Rss,
        "web" => FileKind::Web,
        _ => FileKind::File,
    };
    let source_kind = match current.source_kind.as_deref() {
        Some("yuque") => Some(SourceKind::Yuque),
        Some("meeting") => Some(SourceKind::Meeting),
        Some("misc") => Some(SourceKind::Misc),
        _ => None,
    };
    Ok(Source {
        name: next_name.to_string(),
        kind,
        source_kind,
        url: next_location
            .starts_with("http://")
            .then(|| next_location.to_string())
            .or_else(|| next_location.starts_with("https://").then(|| next_location.to_string())),
        path: (!next_location.starts_with("http://") && !next_location.starts_with("https://"))
            .then(|| next_location.to_string()),
        tags: serde_json::from_str(&current.tags_json).unwrap_or_default(),
        added_at: String::new(),
        updated_at: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
    })
}

/// Remove a Lance-primary registration and its derived binding. Compatibility
/// YAML is only rewritten after the primary mutation has succeeded.
pub fn remove_registration(
    repo_root: &Path,
    project_path: &Path,
    name_or_path: &str,
    keep_file: bool,
    force: bool,
    dry_run: bool,
) -> Result<SourceRemoveReport> {
    let config = ResolvedSourceConfig::from_config(
        crate::service::repo::load_manifest(&repo_root.join("minds.yaml"))?.source.as_ref(),
    )?;
    let store = sync::open_active_store(repo_root)?;
    let catalog = SourceCatalog::discover(&config, repo_root)?;
    let project_rel = project_path.strip_prefix(repo_root).unwrap_or(project_path).to_string_lossy().replace('\\', "/");
    let row = catalog
        .registrations(Some(&store))?
        .into_iter()
        .find(|row| {
            row.project_path == project_rel
                && (row.source_identity == name_or_path || row.registered_location == name_or_path)
        })
        .ok_or_else(|| MfError::usage(format!("source '{name_or_path}' not found"), None))?;
    let kind = match row.source_type.as_str() {
        "pdf" => FileKind::Pdf,
        "rss" => FileKind::Rss,
        "web" => FileKind::Web,
        _ => FileKind::File,
    };
    let is_url = row.registered_location.starts_with("http://") || row.registered_location.starts_with("https://");
    let source = Source {
        name: row.source_identity.clone(),
        kind,
        source_kind: None,
        url: is_url.then(|| row.registered_location.clone()),
        path: (!is_url).then(|| row.registered_location.clone()),
        tags: serde_json::from_str(&row.tags_json).unwrap_or_default(),
        added_at: String::new(),
        updated_at: String::new(),
    };
    let index = crate::service::index::load(project_path)?;
    let references = crate::service::lifecycle::scan_references(
        project_path,
        &index,
        crate::model::lifecycle::ObjectKind::Source,
        &source.name,
    );
    if !references.is_empty() && !force {
        return Err(MfError::usage(
            format!("source '{}' is referenced; use --force to remove anyway", source.name),
            None,
        ));
    }
    let path = source.path.as_ref().map(|path| project_path.join(path));
    let file_deleted = !keep_file && path.as_ref().is_some_and(|path| path.exists());
    if !dry_run {
        store.clear_content_bindings(&std::collections::BTreeSet::from([row.registration_key.clone()]))?;
        store.delete_rows("registrations", &format!("registration_key = '{}'", row.registration_key))?;
        if file_deleted {
            std::fs::remove_file(path.expect("checked"))?;
        }
        super::compatibility::export_project(repo_root, &row.project_identity, false)?;
    }
    Ok(SourceRemoveReport { source, file_deleted, references, side_effects: vec![], force, dry_run })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::manifest::{SearchDefaultMode, SourceBackend};

    #[test]
    fn update_writes_lance_primary_before_exporting_projection() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("projects/alpha");
        std::fs::create_dir_all(project.join("sources")).unwrap();
        std::fs::write(project.join("sources/notes.md"), "# notes").unwrap();
        std::fs::write(
            project.join("mind-index.yaml"),
            "project: alpha\nsources:\n  - name: notes\n    kind: file\n    path: sources/notes.md\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("minds.yaml"), "schema_version: '1'\nprojects: []\n").unwrap();
        let legacy = ResolvedSourceConfig {
            backend: SourceBackend::Legacy,
            is_lance_active: false,
            is_marker_corrupt: false,
            activation_snapshot_id: None,
            storage_schema_version: None,
            chunk_tokens: 384,
            chunk_overlap: 48,
            default_search_mode: SearchDefaultMode::Basic,
        };
        super::super::activation::activate(dir.path(), &legacy).unwrap();

        let source = update_registration(dir.path(), &project, "notes", Some("renamed"), None).unwrap();
        assert_eq!(source.name, "renamed");
        let config = ResolvedSourceConfig::from_config(
            crate::service::repo::load_manifest(&dir.path().join("minds.yaml")).unwrap().source.as_ref(),
        )
        .unwrap();
        let store = sync::open_active_store(dir.path()).unwrap();
        let catalog = SourceCatalog::discover(&config, dir.path()).unwrap();
        let primary = catalog.registrations(Some(&store)).unwrap();
        assert_eq!(primary[0].source_identity, "renamed");
        assert!(std::fs::read_to_string(project.join("mind-index.yaml")).unwrap().contains("name: renamed"));

        let removed = remove_registration(dir.path(), &project, "renamed", false, false, false).unwrap();
        assert!(removed.file_deleted);
        assert!(!project.join("sources/notes.md").exists());
        assert!(catalog.registrations(Some(&store)).unwrap().is_empty());
        assert!(!std::fs::read_to_string(project.join("mind-index.yaml")).unwrap().contains("name: renamed"));
    }
}
