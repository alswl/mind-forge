use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;
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

    // Build an EnvFilter so we can pin noisy dependencies (LanceDB / Arrow /
    // reqwest / …) to WARN while keeping mf at the user-requested level.
    // The RUST_LOG env var takes precedence when set.
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        let level_str = level_to_str(level);
        EnvFilter::new(format!(
            "{level_str},lancedb=warn,lance=warn,lance_index=warn,arrow=warn,datafusion=warn,reqwest=warn"
        ))
    });

    // Diagnostics go to stderr so stdout stays a clean machine-readable
    // contract (JSON envelopes).
    let _ = FmtSubscriber::builder()
        .with_max_level(level)
        .with_writer(std::io::stderr)
        .with_env_filter(filter)
        .without_time()
        .try_init();
    Ok(())
}

fn level_to_str(level: LevelFilter) -> &'static str {
    match level {
        LevelFilter::TRACE => "trace",
        LevelFilter::DEBUG => "debug",
        LevelFilter::INFO => "info",
        LevelFilter::WARN => "warn",
        LevelFilter::ERROR => "error",
        LevelFilter::OFF => "off",
    }
}
