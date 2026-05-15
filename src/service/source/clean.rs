use std::path::Path;

use crate::error::{MfError, Result};
use crate::model::source::{FileKind, Source, SourceIndexEntry, SourceIndexReport};
use crate::service::config as config_svc;
use crate::service::index;

/// Clean dirty entries from the index: removes pdf/file sources whose archive
/// files no longer exist on disk. URL sources are always kept.
pub fn clean(project_path: &Path, dry_run: bool) -> Result<SourceIndexReport> {
    let paths = config_svc::project_paths(project_path)?;
    let sources_dir = project_path.join(&paths.sources);
    if !sources_dir.exists() {
        return Err(MfError::usage(
            format!("project has no {}/ directory at '{}'", paths.sources, sources_dir.display()),
            Some("use 'mf project lint --fix' to create missing directories".to_string()),
        ));
    }

    let mut index = index::load(project_path)?;
    let index_sources = index.sources.unwrap_or_default();

    let mut kept: Vec<Source> = Vec::new();
    let mut removed: Vec<SourceIndexEntry> = Vec::new();

    for s in index_sources {
        match s.kind {
            FileKind::Auto | FileKind::Rss | FileKind::Web => {
                kept.push(s);
            }
            FileKind::Pdf | FileKind::File => {
                let path = s.path.as_ref().ok_or_else(|| {
                    MfError::Internal(anyhow::anyhow!("file-type source '{}' has no path field", s.name))
                })?;
                let exists = project_path.join(path).exists();
                if exists {
                    kept.push(s);
                } else {
                    removed.push(SourceIndexEntry { name: s.name.clone(), kind: s.kind.clone(), path: path.clone() });
                }
            }
        }
    }

    let kept_count = kept.len() as u64;
    let dry_run_value = dry_run;

    if !dry_run && !removed.is_empty() {
        kept.sort_by(|a, b| a.name.cmp(&b.name));
        index.sources = Some(kept);
        index::save(project_path, &index)?;
    }

    removed.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(SourceIndexReport { added: vec![], removed, kept_count, dry_run: dry_run_value })
}
