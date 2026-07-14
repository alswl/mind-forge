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

    // Diagnostics go to stderr so stdout stays a clean machine-readable
    // contract (JSON envelopes). Dependencies such as LanceDB emit INFO-level
    // tracing events that would otherwise corrupt `--output json` stdout.
    let _ = FmtSubscriber::builder().with_max_level(level).without_time().with_writer(std::io::stderr).try_init();
    Ok(())
}
