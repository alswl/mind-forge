use crate::error::Result;
use crate::output::Format;
use crate::CommandOutcome;

pub fn handle_version(format: Format) -> Result<CommandOutcome> {
    let version = env!("CARGO_PKG_VERSION");
    let commit = env!("CARGO_GIT_SHA");
    let build_date = env!("CARGO_BUILD_DATE");
    let rustc = env!("CARGO_RUSTC");
    let target_triple = format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS);

    match format {
        Format::Json => {
            let data = serde_json::json!({
                "version": version,
                "commit": commit,
                "build_date": build_date,
                "rustc": rustc,
                "target_triple": target_triple,
            });
            Ok(CommandOutcome::Success(data, None))
        }
        Format::Text => {
            let line = format!("mf {version} ({commit}, built {build_date}, rustc {rustc})");
            Ok(CommandOutcome::Raw(line + "\n", None))
        }
    }
}
