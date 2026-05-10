use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;

use super::{create_symlink, infer_kind, sha256_file};
use crate::error::{MfError, Result};
use crate::model::asset::Asset;
use crate::service::index;

/// Parameters for `add()`.
pub struct AddArgs {
    pub source: PathBuf,
    pub tags: Vec<String>,
    pub link_mode: bool,
    pub force: bool,
}

/// Add an external file to the project's asset pool.
pub fn add(project_path: &Path, cwd: &Path, args: &AddArgs) -> Result<Asset> {
    let source_canonical = cwd.join(&args.source).canonicalize().map_err(MfError::Io)?;

    let metadata = fs::metadata(&source_canonical).map_err(MfError::Io)?;
    if !metadata.is_file() {
        return Err(MfError::usage(
            format!("source path '{}' must be an existing regular file", args.source.display()),
            None as Option<String>,
        ));
    }

    let assets_dir = project_path.join("assets");
    fs::create_dir_all(&assets_dir).map_err(MfError::Io)?;

    let basename = args.source.file_name().map(|s| s.to_string_lossy().to_string()).ok_or_else(|| {
        MfError::usage(format!("cannot extract filename from '{}'", args.source.display()), None as Option<String>)
    })?;
    let dest = assets_dir.join(&basename);

    let assets_canonical = assets_dir.canonicalize().map_err(MfError::Io)?;
    if source_canonical.starts_with(&assets_canonical) {
        return Err(MfError::usage(
            "source file is already inside the project's assets/ directory",
            Some("use 'mf asset update <PATH>' to refresh metadata".to_string()),
        ));
    }

    if dest.exists() && !args.force {
        return Err(MfError::file_exists(dest.canonicalize().unwrap_or(dest)));
    }

    if args.link_mode {
        if dest.exists() {
            fs::remove_file(&dest).map_err(MfError::Io)?;
        }
        create_symlink(&source_canonical, &dest)?;
    } else {
        if dest.exists() {
            fs::remove_file(&dest).map_err(MfError::Io)?;
        }
        fs::copy(&source_canonical, &dest).map_err(MfError::Io)?;
    }

    let dest_metadata = fs::metadata(&dest).map_err(MfError::Io)?;
    let size = dest_metadata.len();
    let hash = sha256_file(&dest)?;

    let rel_path = format!("assets/{basename}");

    let ext = dest.extension();
    let kind = infer_kind(ext);

    let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let asset =
        Asset { name: basename, kind, path: rel_path.clone(), size, hash, tags: args.tags.clone(), added_at: now };

    let mut index = index::load(project_path)?;
    let assets = index.assets.get_or_insert_with(Vec::new);

    if args.force {
        assets.retain(|a| a.path != rel_path);
    }
    assets.push(asset.clone());
    assets.sort_by(|a, b| a.name.cmp(&b.name));

    index::save(project_path, &index)?;

    Ok(asset)
}
