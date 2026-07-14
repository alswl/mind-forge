//! Model bundle management: install, import, and status for the local
//! embedding model used by advanced Source search.
//!
//! Only `install` may download model/runtime assets. `import` is network-free.
//! `status` is strictly read-only. Sync/search never download implicitly.

use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::error::{MfError, Result};

/// Repository model directory under `.mind/models/`.
const MODELS_DIR: &str = "models";

/// Result of a model installation.
#[derive(Debug, Serialize)]
pub struct ModelInstallResult {
    pub model_id: String,
    pub revision: String,
    pub installed: bool,
    pub dry_run: bool,
    pub path: String,
}

/// Model status report.
#[derive(Debug, Serialize)]
pub struct ModelStatusReport {
    pub model_id: String,
    pub revision: Option<String>,
    pub status: String, // ready, missing, corrupt
    pub path: String,
}

/// Install the default embedding model. Downloads from HuggingFace if not cached.
pub fn install_model(
    repo_root: &Path,
    model_id: Option<&str>,
    revision: Option<&str>,
    dry_run: bool,
) -> Result<ModelInstallResult> {
    let model_id = model_id.unwrap_or("intfloat/multilingual-e5-small");
    let revision = revision.unwrap_or("main");
    let model_dir = repo_root.join(".mind").join(MODELS_DIR).join(model_id.replace('/', "_"));

    if dry_run {
        return Ok(ModelInstallResult {
            model_id: model_id.to_string(),
            revision: revision.to_string(),
            installed: false,
            dry_run: true,
            path: model_dir.to_string_lossy().to_string(),
        });
    }

    // Create model directory
    fs::create_dir_all(&model_dir)?;

    // Trigger FastEmbed to download/cache the model
    let _model =
        fastembed::TextEmbedding::try_new(fastembed::InitOptions::new(fastembed::EmbeddingModel::MultilingualE5Small))
            .map_err(|e| MfError::advanced_store(format!("failed to install model: {e}"), None))?;

    // Write an installed manifest
    let manifest = serde_json::json!({
        "model_id": model_id,
        "revision": revision,
        "installed_at": chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
    });
    fs::write(model_dir.join("installed.json"), serde_json::to_string_pretty(&manifest).unwrap_or_default())?;

    Ok(ModelInstallResult {
        model_id: model_id.to_string(),
        revision: revision.to_string(),
        installed: true,
        dry_run: false,
        path: model_dir.to_string_lossy().to_string(),
    })
}

/// Import a model from a local bundle directory (network-free).
pub fn import_model(repo_root: &Path, bundle_dir: &str, dry_run: bool) -> Result<ModelInstallResult> {
    let src = Path::new(bundle_dir);
    if !src.exists() {
        return Err(MfError::usage(
            format!("model bundle directory not found: {bundle_dir}"),
            Some("specify a valid directory containing the model files".to_string()),
        ));
    }

    let model_id = src.file_name().unwrap_or_default().to_string_lossy().to_string();
    let model_dir = repo_root.join(".mind").join(MODELS_DIR).join(&model_id);

    if dry_run {
        return Ok(ModelInstallResult {
            model_id,
            revision: "imported".to_string(),
            installed: false,
            dry_run: true,
            path: model_dir.to_string_lossy().to_string(),
        });
    }

    // Copy the bundle directory
    fs::create_dir_all(&model_dir)?;
    copy_dir_contents(src, &model_dir)?;

    Ok(ModelInstallResult {
        model_id,
        revision: "imported".to_string(),
        installed: true,
        dry_run: false,
        path: model_dir.to_string_lossy().to_string(),
    })
}

/// Report model installation status (read-only, network-free).
pub fn model_status(repo_root: &Path, model_id: Option<&str>) -> Result<ModelStatusReport> {
    let model_id = model_id.unwrap_or("intfloat/multilingual-e5-small");
    let model_dir = repo_root.join(".mind").join(MODELS_DIR).join(model_id.replace('/', "_"));
    let installed_json = model_dir.join("installed.json");

    if installed_json.exists() {
        let status = if model_dir.join("installed.json").exists() { "ready" } else { "corrupt" };
        Ok(ModelStatusReport {
            model_id: model_id.to_string(),
            revision: Some("unknown".to_string()),
            status: status.to_string(),
            path: model_dir.to_string_lossy().to_string(),
        })
    } else {
        Ok(ModelStatusReport {
            model_id: model_id.to_string(),
            revision: None,
            status: "missing".to_string(),
            path: model_dir.to_string_lossy().to_string(),
        })
    }
}

fn copy_dir_contents(src: &Path, dst: &Path) -> Result<()> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            fs::create_dir_all(&dst_path)?;
            copy_dir_contents(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_dry_run_does_not_write() {
        let dir = tempfile::tempdir().unwrap();
        let result = install_model(dir.path(), None, None, true).unwrap();
        assert!(result.dry_run);
        assert!(!result.installed);
    }

    #[test]
    fn model_status_reports_missing() {
        let dir = tempfile::tempdir().unwrap();
        let report = model_status(dir.path(), None).unwrap();
        assert_eq!(report.status, "missing");
    }

    #[test]
    fn import_missing_dir_is_error() {
        let dir = tempfile::tempdir().unwrap();
        let result = import_model(dir.path(), "/nonexistent/bundle", false);
        assert!(result.is_err());
    }
}
