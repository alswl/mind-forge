pub mod config_path;
pub mod logging;
pub mod repo;

use std::path::PathBuf;

use crate::cli::GlobalOpts;
use crate::error::{MfError, Result};
use crate::output::Format;

#[derive(Debug, Clone)]
pub struct AppContext {
    format: Format,
    config_path: PathBuf,
    repo_root: Option<PathBuf>,
    cwd: PathBuf,
    project: Option<String>,
    color: bool,
    quiet: bool,
}

impl AppContext {
    pub fn from_global_opts(global: &GlobalOpts) -> Result<Self> {
        logging::validate(global)?;
        logging::init(global)?;
        let config_path = config_path::resolve(global.config.as_ref())?;

        let cwd = std::env::current_dir().map_err(MfError::Io)?;

        let repo_root = if let Some(ref root) = global.root {
            let canonical = root.canonicalize().map_err(|_| MfError::not_in_mind_repo())?;
            if !canonical.join("minds.yaml").exists() {
                return Err(MfError::not_in_mind_repo());
            }
            tracing::debug!("repo root via --root: {}", canonical.display());
            Some(canonical)
        } else if let Some(ref cfg) = global.config {
            if cfg.exists() { repo::detect_repo_root_with_config(cfg) } else { None }
        } else {
            repo::detect_repo_root(&cwd, crate::defaults::MAX_REPO_SEARCH_DEPTH)
        };

        let color = !global.no_color;
        let quiet = global.quiet;
        let project = global.project.clone();

        Ok(Self { format: global.effective_format(), config_path, repo_root, cwd, project, color, quiet })
    }

    // ── Accessor methods (T005) ──

    pub fn format(&self) -> Format {
        self.format
    }

    pub fn config_path(&self) -> &PathBuf {
        &self.config_path
    }

    pub fn repo_root(&self) -> Option<&PathBuf> {
        self.repo_root.as_ref()
    }

    pub fn cwd(&self) -> &PathBuf {
        &self.cwd
    }

    /// Returns the project name; only valid when a repo_root is present.
    pub fn project(&self) -> Option<&str> {
        if self.repo_root.is_some() { self.project.as_deref() } else { None }
    }

    pub fn color(&self) -> bool {
        self.color
    }

    pub fn quiet(&self) -> bool {
        self.quiet
    }

    /// Checks repo_root; returns a NotInMindRepo error when it is None.
    pub fn require_repo(&self) -> Result<()> {
        if self.repo_root.is_none() {
            return Err(MfError::not_in_mind_repo());
        }
        Ok(())
    }

    /// Like require_repo but returns the &Path on success.
    pub fn require_repo_path(&self) -> Result<&PathBuf> {
        self.repo_root.as_ref().ok_or_else(MfError::not_in_mind_repo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_global_opts_populates_context_fields() {
        let global = GlobalOpts {
            root: None,
            config: None,
            verbose: 0,
            quiet: true,
            format: Format::Json,
            json: false,
            no_color: true,
            project: Some("demo".to_string()),
        };
        let ctx = AppContext::from_global_opts(&global).unwrap();
        assert_eq!(ctx.format(), Format::Json);
        assert!(ctx.quiet(), "quiet should be true");
        assert!(!ctx.color(), "color should be false when no_color is true");
        assert_eq!(ctx.project(), None, "project is None without a repo_root");
    }
}
