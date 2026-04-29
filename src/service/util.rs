//! Shared utilities: atomic write, schema version validation.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{MfError, Result};

/// Atomically write content to a file using write-then-rename.
///
/// Writes to a temporary file (`{path}.yaml.tmp.{pid}.{rand}`) then
/// renames atomically, ensuring partial writes never land at `path`.
pub fn atomic_write(path: &Path, content: &str) -> Result<()> {
    let tmp_path = {
        let pid = std::process::id();
        let rand: u64 = {
            let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
            t.as_nanos() as u64
        };
        path.with_extension(format!("yaml.tmp.{pid}.{rand}"))
    };
    fs::write(&tmp_path, content).map_err(MfError::Io)?;
    fs::rename(&tmp_path, path).map_err(MfError::Io)?;
    Ok(())
}

/// Validate that `version` is among the compatible schema versions.
///
/// The compatible set is `["1"]` for the current generation.
pub fn validate_schema_version(version: &str, path: &Path) -> Result<()> {
    let compatible = ["1"];
    if !compatible.contains(&version) {
        return Err(MfError::IncompatibleSchema {
            path: path.to_path_buf(),
            found: version.to_string(),
            expected: compatible.iter().map(|s| s.to_string()).collect(),
        });
    }
    Ok(())
}

/// Canonicalize a path, falling back to the raw path on failure.
///
/// Canonicalization failures are logged as a debug warning so callers
/// are not silently operating on unchecked paths.
pub fn try_canonicalize(path: &Path) -> PathBuf {
    match path.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            // Canonicalization can fail when the path does not exist or
            // a component is inaccessible.  The caller must still have
            // a path to work with, so we fall back to the raw path.
            tracing::debug!("canonicalize failed for {}: {e}; using raw path", path.display());
            path.to_path_buf()
        }
    }
}
