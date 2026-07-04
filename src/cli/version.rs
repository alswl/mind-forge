use crate::cli::CommandCtx;
use crate::error::Result;
use crate::output::Format;
use crate::CommandOutcome;

pub fn handle_version(ctx: &CommandCtx) -> Result<CommandOutcome> {
    let version = env!("CARGO_PKG_VERSION");
    let commit = env!("CARGO_GIT_SHA");
    let build_date = env!("CARGO_BUILD_DATE");
    let rustc = env!("CARGO_RUSTC");
    let target_triple = format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS);
    let full_version = format!("{version}-dev+{commit}");

    match ctx.format() {
        Format::Json => {
            let data = serde_json::json!({
                "version": full_version,
                "commit": commit,
                "build_date": build_date,
                "rustc": rustc,
                "target_triple": target_triple,
            });
            Ok(CommandOutcome::Success(data, Vec::new(), None))
        }
        Format::Text => {
            let line = format!("mf {full_version} (built {build_date}, rustc {rustc})");
            Ok(CommandOutcome::Raw(line + "\n", None))
        }
    }
}
