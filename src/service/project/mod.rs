use std::path::Path;

pub use self::archive::archive_project;
pub use self::import::import_project;
pub use self::index::{resolve_project, status_for};
pub use self::lifecycle::{lint_project, lint_repo};
pub use self::list::list_projects;
pub(crate) use self::new::{scaffold, upsert_project_entry};
pub use self::remove::remove_project;
pub use self::rename::rename_project;
pub use self::show::show;

pub mod remove;
pub mod rename;

/// Read a project's mind-index.yaml and return (document_count, last_activity_at).
pub(crate) fn read_index_counts(project_path: &Path) -> (u64, Option<String>) {
    let index = match crate::service::index::load(project_path) {
        Ok(idx) => idx,
        Err(e) => {
            let detail = match &e {
                crate::error::MfError::ParseError { detail, .. } => format!(": {detail}"),
                _ => String::new(),
            };
            tracing::warn!("failed to load index for {}{detail}", project_path.display());
            crate::model::index::IndexFile::create_default()
        }
    };

    let articles = index.articles.as_ref().map(|v| v.len() as u64).unwrap_or(0);
    let assets = index.assets.as_ref().map(|v| v.len() as u64).unwrap_or(0);
    let sources = index.sources.as_ref().map(|v| v.len() as u64).unwrap_or(0);
    let terms = index.terms.as_ref().map(|v| v.len() as u64).unwrap_or(0);
    let count = articles + assets + sources + terms;

    let mut max_ts: Option<String> = None;
    for entry in index.articles.iter().flatten() {
        if max_ts.as_ref().map_or(true, |m| &entry.updated_at > m) {
            max_ts = Some(entry.updated_at.clone());
        }
    }
    for entry in index.assets.iter().flatten() {
        if max_ts.as_ref().map_or(true, |m| &entry.added_at > m) {
            max_ts = Some(entry.added_at.clone());
        }
    }
    for entry in index.sources.iter().flatten() {
        if max_ts.as_ref().map_or(true, |m| &entry.updated_at > m) {
            max_ts = Some(entry.updated_at.clone());
        }
    }

    (count, max_ts)
}

pub mod archive;
pub mod import;
pub mod index;
pub mod lifecycle;
pub mod list;
pub mod new;
pub mod show;
