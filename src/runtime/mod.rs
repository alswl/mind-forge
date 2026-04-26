pub mod color;
pub mod config_path;
pub mod logging;

use std::path::PathBuf;

use crate::cli::GlobalOpts;
use crate::error::Result;
use crate::output::Format;

#[derive(Debug, Clone)]
pub struct AppContext {
    pub format: Format,
    pub config_path: PathBuf,
    pub color_enabled: bool,
}

impl AppContext {
    pub fn from_global_opts(global: &GlobalOpts) -> Result<Self> {
        logging::validate(global)?;
        logging::init(global)?;
        let config_path = config_path::resolve(global.config.as_ref())?;
        let color_enabled = color::is_color_enabled(global);

        Ok(Self { format: global.format, config_path, color_enabled })
    }
}
