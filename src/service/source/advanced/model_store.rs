//! Model bundle management: install, import, and status for the local
//! embedding model used by advanced Source search.
//!
//! Only `install` may download model/runtime assets. `import` is network-free.
//! `status` is strictly read-only. Sync/search never download implicitly.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{MfError, Result};

/// Repository model directory under `.mind/models/`.
const MODELS_DIR: &str = "models";
const DEFAULT_MODEL_ID: &str = "intfloat/multilingual-e5-small";
const DEFAULT_REVISION: &str = "main";
const MODEL_FILES: &[&str] =
    &["onnx/model.onnx", "tokenizer.json", "config.json", "special_tokens_map.json", "tokenizer_config.json"];

#[derive(Debug, Serialize, Deserialize)]
struct InstalledManifest {
    model_id: String,
    revision: String,
    installed_at: String,
}

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

/// Read-only diagnostics for dependencies required by vector retrieval.
#[derive(Debug, Serialize)]
pub struct ModelDoctorReport {
    pub model_status: String,
    pub model_path: String,
    pub runtime_path: Option<String>,
    pub runtime_status: String,
    pub ort_dylib_path: Option<String>,
    pub ready: bool,
}

/// Install the default embedding model. Downloads from HuggingFace if not cached.
pub fn install_model(
    repo_root: &Path,
    model_id: Option<&str>,
    revision: Option<&str>,
    dry_run: bool,
) -> Result<ModelInstallResult> {
    let model_id = model_id.unwrap_or(DEFAULT_MODEL_ID);
    let revision = revision.unwrap_or(DEFAULT_REVISION);
    ensure_supported_model(model_id)?;
    let model_dir = model_dir(repo_root);

    if dry_run {
        return Ok(ModelInstallResult {
            model_id: model_id.to_string(),
            revision: revision.to_string(),
            installed: false,
            dry_run: true,
            path: model_dir.to_string_lossy().to_string(),
        });
    }

    let staging = staging_dir(&model_dir);
    fs::create_dir_all(&staging)?;
    let cache_dir = staging.join(".hf-cache");
    let endpoint = std::env::var("HF_ENDPOINT").unwrap_or_else(|_| "https://huggingface.co".to_string());
    let api = hf_hub::api::sync::ApiBuilder::new()
        .with_cache_dir(cache_dir)
        .with_endpoint(endpoint)
        .with_progress(true)
        .build()
        .map_err(|e| MfError::advanced_store(format!("failed to initialize model downloader: {e}"), None))?;
    let repo =
        api.repo(hf_hub::Repo::with_revision(model_id.to_string(), hf_hub::RepoType::Model, revision.to_string()));
    for file in MODEL_FILES {
        let downloaded = repo
            .get(file)
            .map_err(|e| MfError::advanced_store(format!("failed to download model file {file}: {e}"), None))?;
        copy_file(&downloaded, &staging.join(file))?;
    }
    fs::remove_dir_all(staging.join(".hf-cache"))?;
    write_manifest(&staging, model_id, revision)?;
    replace_model_dir(&staging, &model_dir)?;

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

    validate_bundle(src)?;
    let model_id = DEFAULT_MODEL_ID.to_string();
    let model_dir = model_dir(repo_root);

    if dry_run {
        return Ok(ModelInstallResult {
            model_id,
            revision: "imported".to_string(),
            installed: false,
            dry_run: true,
            path: model_dir.to_string_lossy().to_string(),
        });
    }

    let staging = staging_dir(&model_dir);
    fs::create_dir_all(&staging)?;
    for file in MODEL_FILES {
        copy_file(&src.join(file), &staging.join(file))?;
    }
    write_manifest(&staging, &model_id, "imported")?;
    replace_model_dir(&staging, &model_dir)?;

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
    let model_id = model_id.unwrap_or(DEFAULT_MODEL_ID);
    ensure_supported_model(model_id)?;
    let model_dir = model_dir(repo_root);
    let installed_json = model_dir.join("installed.json");

    if installed_json.exists() {
        let manifest: InstalledManifest = fs::read(&installed_json)
            .ok()
            .and_then(|data| serde_json::from_slice(&data).ok())
            .ok_or_else(|| MfError::advanced_store("model manifest is invalid".to_string(), None))?;
        let status = if bundle_is_valid(&model_dir) { "ready" } else { "corrupt" };
        Ok(ModelStatusReport {
            model_id: model_id.to_string(),
            revision: Some(manifest.revision),
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

/// Detect model and ONNX Runtime availability without loading, downloading, or mutating.
pub fn doctor_model(repo_root: &Path, model_id: Option<&str>) -> Result<ModelDoctorReport> {
    let status = model_status(repo_root, model_id)?;
    let dir = PathBuf::from(&status.path);
    let runtime = runtime_library_path(&dir);
    let env_runtime = std::env::var("ORT_DYLIB_PATH").ok().filter(|path| Path::new(path).is_file());
    let runtime_path = runtime.or_else(|| env_runtime.clone());
    let runtime_status = if runtime_path.is_some() { "ready" } else { "missing" }.to_string();
    Ok(ModelDoctorReport {
        ready: status.status == "ready" && runtime_path.is_some(),
        model_status: status.status,
        model_path: status.path,
        runtime_path,
        runtime_status,
        ort_dylib_path: env_runtime,
    })
}

fn runtime_library_path(model_dir: &Path) -> Option<String> {
    let name = if cfg!(target_os = "windows") {
        "onnxruntime.dll"
    } else if cfg!(target_os = "macos") {
        "libonnxruntime.dylib"
    } else {
        "libonnxruntime.so"
    };
    let path = model_dir.join("runtime").join(name);
    path.is_file().then(|| path.to_string_lossy().to_string())
}

fn ensure_supported_model(model_id: &str) -> Result<()> {
    if model_id == DEFAULT_MODEL_ID {
        Ok(())
    } else {
        Err(MfError::usage(
            format!("unsupported embedding model: {model_id}"),
            Some(format!("only {DEFAULT_MODEL_ID} is currently supported")),
        ))
    }
}

fn model_dir(repo_root: &Path) -> PathBuf {
    repo_root.join(".mind").join(MODELS_DIR).join(DEFAULT_MODEL_ID.replace('/', "_"))
}

fn staging_dir(model_dir: &Path) -> PathBuf {
    model_dir.with_file_name(format!(
        ".{}.staging-{}",
        model_dir.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id()
    ))
}

fn copy_file(src: &Path, dst: &Path) -> Result<()> {
    let parent = dst.parent().ok_or_else(|| MfError::advanced_store("invalid model destination".to_string(), None))?;
    fs::create_dir_all(parent)?;
    fs::copy(src, dst)?;
    Ok(())
}

fn write_manifest(dir: &Path, model_id: &str, revision: &str) -> Result<()> {
    let manifest = InstalledManifest {
        model_id: model_id.to_string(),
        revision: revision.to_string(),
        installed_at: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
    };
    fs::write(dir.join("installed.json"), serde_json::to_vec_pretty(&manifest)?)?;
    Ok(())
}

fn bundle_is_valid(dir: &Path) -> bool {
    MODEL_FILES.iter().all(|file| dir.join(file).is_file())
}

fn validate_bundle(dir: &Path) -> Result<()> {
    if bundle_is_valid(dir) {
        Ok(())
    } else {
        Err(MfError::usage(
            format!("invalid model bundle: {}", dir.display()),
            Some("bundle must contain onnx/model.onnx and the four tokenizer JSON files".to_string()),
        ))
    }
}

fn replace_model_dir(staging: &Path, model_dir: &Path) -> Result<()> {
    if model_dir.exists() {
        fs::remove_dir_all(model_dir)?;
    }
    fs::rename(staging, model_dir)?;
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

    #[test]
    fn import_uses_canonical_model_directory_and_becomes_ready() {
        let repo = tempfile::tempdir().unwrap();
        let bundle = tempfile::tempdir().unwrap();
        for file in MODEL_FILES {
            let path = bundle.path().join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "test").unwrap();
        }

        import_model(repo.path(), &bundle.path().display().to_string(), false).unwrap();
        let report = model_status(repo.path(), None).unwrap();
        assert_eq!(report.status, "ready");
        assert!(Path::new(&report.path).join("onnx/model.onnx").is_file());
    }
}
