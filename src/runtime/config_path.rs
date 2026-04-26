use std::path::{Path, PathBuf};

use anyhow::Context;
use directories::ProjectDirs;

use crate::error::Result;

pub fn resolve(override_path: Option<&PathBuf>) -> Result<PathBuf> {
    if let Some(path) = override_path {
        return Ok(path.clone());
    }

    let project_dirs = ProjectDirs::from("", "", "mf")
        .context("unable to determine platform configuration directory for mf")?;
    Ok(default_config_path(project_dirs.config_dir()))
}

fn default_config_path(config_dir: &Path) -> PathBuf {
    config_dir.join("config.toml")
}
