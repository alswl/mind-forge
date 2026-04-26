use tracing::level_filters::LevelFilter;
use tracing_subscriber::FmtSubscriber;

use crate::cli::GlobalOpts;
use crate::error::{MfError, Result};

pub fn validate(global: &GlobalOpts) -> Result<()> {
    if global.verbose > 0 && global.quiet {
        return Err(MfError::usage("'--verbose' cannot be used with '--quiet'", None));
    }
    Ok(())
}

pub fn init(global: &GlobalOpts) -> Result<()> {
    let level = if global.quiet {
        LevelFilter::ERROR
    } else {
        match global.verbose {
            0 => LevelFilter::INFO,
            1 => LevelFilter::DEBUG,
            _ => LevelFilter::TRACE,
        }
    };

    let _ = FmtSubscriber::builder().with_max_level(level).without_time().try_init();
    Ok(())
}
