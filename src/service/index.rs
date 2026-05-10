use std::fs;
use std::io;
use std::path::Path;

use crate::error::{MfError, Result};
use crate::model::index::IndexFile;
use crate::service::util::{atomic_write, validate_schema_version};

const INDEX_FILENAME: &str = "mind-index.yaml";

/// Load `mind-index.yaml` from `project_root`.
///
/// Returns `IndexFile::create_default()` if the file does not exist.
/// Otherwise reads, parses YAML, and validates `schema_version`.
pub fn load(project_root: &Path) -> Result<IndexFile> {
    let path = project_root.join(INDEX_FILENAME);
    let content = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Ok(IndexFile::create_default());
        }
        Err(e) => return Err(MfError::from(e)),
    };
    let index: IndexFile = serde_yaml::from_str(&content).map_err(|e| MfError::ParseError {
        kind: "yaml".to_string(),
        path: path.clone(),
        detail: e.to_string(),
    })?;
    validate_schema_version(&index.schema_version, &path)?;
    Ok(index)
}

/// Atomically write `index` to `project_root/mind-index.yaml`.
pub fn save(project_root: &Path, index: &IndexFile) -> Result<()> {
    let path = project_root.join(INDEX_FILENAME);
    let yaml = serde_yaml::to_string(index)
        .map_err(|e| MfError::Internal(anyhow::anyhow!("serialize {}: {e}", path.display())))?;
    atomic_write(&path, &yaml)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_returns_default_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let index = load(dir.path()).unwrap();
        assert_eq!(index, IndexFile::create_default());
    }

    #[test]
    fn load_returns_incompatible_schema_on_unsupported_version() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("mind-index.yaml"), "schema_version: \"99\"\n").unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(matches!(err, MfError::IncompatibleSchema { .. }));
        assert_eq!(err.kind(), "incompatible-schema");
    }

    #[test]
    fn load_returns_parse_error_on_corrupt_yaml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("mind-index.yaml"), "not: : valid: yaml").unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(matches!(err, MfError::ParseError { .. }));
        assert_eq!(err.kind(), "parse-error");
    }

    #[test]
    fn save_returns_io_on_readonly_parent() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let dir = tempfile::tempdir().unwrap();
            let mut perms = std::fs::metadata(dir.path()).unwrap().permissions();
            perms.set_mode(0o555);
            std::fs::set_permissions(dir.path(), perms).unwrap();
            let err = save(dir.path(), &IndexFile::create_default()).unwrap_err();
            assert!(matches!(err, MfError::Io(_)));
            // Restore so tempdir can clean up
            let mut perms = std::fs::metadata(dir.path()).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(dir.path(), perms).unwrap();
        }
    }
}
