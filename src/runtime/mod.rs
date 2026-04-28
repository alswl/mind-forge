pub mod color;
pub mod config_path;
pub mod logging;
pub mod repo;

use std::path::PathBuf;

use crate::cli::GlobalOpts;
use crate::error::{MfError, Result};
use crate::output::Format;

#[derive(Debug, Clone)]
pub struct AppContext {
    pub format: Format,
    pub config_path: PathBuf,
    pub color_enabled: bool,
    pub repo_root: Option<PathBuf>,
}

impl AppContext {
    pub fn from_global_opts(global: &GlobalOpts) -> Result<Self> {
        logging::validate(global)?;
        logging::init(global)?;
        let config_path = config_path::resolve(global.config.as_ref())?;
        let color_enabled = color::is_color_enabled(global);

        let repo_root = if let Some(ref cfg) = global.config {
            if cfg.exists() {
                repo::detect_repo_root_with_config(cfg)
            } else {
                None
            }
        } else {
            std::env::current_dir().ok().and_then(|cwd| repo::detect_repo_root(&cwd, 50))
        };

        Ok(Self { format: global.format, config_path, color_enabled, repo_root })
    }

    /// 检查 repo_root，为 None 时返回 NotInMindRepo 错误
    pub fn require_repo(&self) -> Result<()> {
        if self.repo_root.is_none() {
            return Err(MfError::not_in_mind_repo());
        }
        Ok(())
    }
}
