use crate::error::{MfError, Result};
use crate::service::index;
use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SourceMoveReport {
    pub name: String,
    pub old_path: String,
    pub new_path: String,
    pub dry_run: bool,
}

pub fn move_source(
    source_project: &Path,
    target_project: &Path,
    selector: &str,
    dry_run: bool,
) -> Result<SourceMoveReport> {
    if source_project == target_project {
        return Err(MfError::usage("source and destination projects must differ", None::<String>));
    }
    let mut source_index = index::load(source_project)?;
    let original_source_index = source_index.clone();
    let source_pos = source_index
        .sources
        .as_ref()
        .and_then(|sources| sources.iter().position(|s| s.name == selector || s.path.as_deref() == Some(selector)))
        .ok_or_else(|| {
            MfError::not_found(
                format!("source '{selector}' not found"),
                Some("use `mf source list` to see available sources".to_string()),
            )
        })?;
    let source = source_index.sources.as_ref().unwrap()[source_pos].clone();
    let old_rel =
        source.path.clone().ok_or_else(|| MfError::usage("only local sources can be moved", None::<String>))?;
    let filename = Path::new(&old_rel)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| MfError::usage("source path has no filename", None::<String>))?;
    let new_rel = format!("sources/{}/{}", source.kind.as_str(), filename);
    let old_full = source_project.join(&old_rel);
    let new_full = target_project.join(&new_rel);
    let mut target_index = index::load(target_project)?;
    if target_index.sources.as_ref().is_some_and(|sources| {
        sources.iter().any(|s| s.name == source.name || s.path.as_deref() == Some(new_rel.as_str()))
    }) {
        return Err(MfError::usage(
            format!("destination already contains source '{}'", source.name),
            Some("choose a different source name or remove the destination entry".to_string()),
        ));
    }
    let report =
        SourceMoveReport { name: source.name.clone(), old_path: old_rel.clone(), new_path: new_rel.clone(), dry_run };
    if dry_run {
        return Ok(report);
    }
    if !old_full.exists() {
        return Err(MfError::not_found(format!("source file '{}' not found", old_full.display()), None::<String>));
    }
    if new_full.exists() {
        return Err(MfError::usage(
            format!("destination source path '{}' already exists", new_rel),
            Some("remove the destination path before moving".to_string()),
        ));
    }
    if let Some(parent) = new_full.parent() {
        fs::create_dir_all(parent).map_err(MfError::Io)?;
    }
    fs::rename(&old_full, &new_full).map_err(MfError::Io)?;
    source_index.sources.as_mut().unwrap().remove(source_pos);
    if let Err(error) = index::save(source_project, &source_index) {
        let _ = fs::rename(&new_full, &old_full);
        return Err(error);
    }
    let mut moved = source;
    moved.path = Some(new_rel);
    target_index.sources.get_or_insert_with(Vec::new).push(moved);
    if let Err(error) = index::save(target_project, &target_index) {
        let _ = fs::rename(&new_full, &old_full);
        let _ = index::save(source_project, &original_source_index);
        return Err(error);
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn moves_local_source_between_project_indexes() {
        let repo = tempfile::tempdir().unwrap();
        let a = repo.path().join("a");
        let b = repo.path().join("b");
        std::fs::create_dir_all(a.join("sources/file")).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(
            a.join("mind-index.yaml"),
            "schema: '1'\nsources:\n  - name: note\n    type: file\n    path: sources/file/note.md\n",
        )
        .unwrap();
        std::fs::write(a.join("sources/file/note.md"), "note\n").unwrap();
        let report = move_source(&a, &b, "note", false).unwrap();
        assert_eq!(report.new_path, "sources/file/note.md");
        assert!(b.join("sources/file/note.md").exists());
        assert!(!a.join("sources/file/note.md").exists());
        assert!(index::load(&a).unwrap().sources.is_none_or(|sources| sources.is_empty()));
        assert_eq!(index::load(&b).unwrap().sources.unwrap()[0].name, "note");
    }
}
