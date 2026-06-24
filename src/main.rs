mod cli;
mod defaults;
mod error;
mod exit;
mod model;
mod output;
mod runtime;
mod service;
use clap::{error::ErrorKind, CommandFactory, Parser};
use cli::deprecation::DeprecationContext;
use cli::{CommandCtx, CommandOutcome, RepoRequirement, RootCli};
use error::{MfError, Result};
use exit::ExitCode;
use output::{render, Payload};
use runtime::AppContext;
use std::{
    ffi::OsString,
    io::{self, Write},
    process::ExitCode as ProcessExitCode,
};
fn main() -> ProcessExitCode {
    match run(std::env::args_os().collect(), &mut io::stdout(), &mut io::stderr()) {
        Ok(code) => ProcessExitCode::from(code as u8),
        Err(err) => {
            let _ = writeln!(io::stderr(), "internal error: {err:#}");
            ProcessExitCode::from(1)
        }
    }
}
fn run(args: Vec<OsString>, stdout: &mut dyn Write, stderr: &mut dyn Write) -> Result<ExitCode> {
    if args.len() <= 1 {
        write_command_help(&mut RootCli::command(), stdout)?;
        return Ok(ExitCode::Ok);
    }
    let cli = match RootCli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(err) => return handle_parse_error(err, &args, stdout, stderr),
    };
    let context = match AppContext::from_global_opts(&cli.global) {
        Ok(context) => context,
        Err(err) => {
            render(stderr, cli.global.format, !cli.global.no_color, Payload::Error(&err))?;
            return Ok(err.exit_code());
        }
    };
    tracing::debug!(config_path = %context.config_path().display(), "resolved config path");
    if cli.requires_repo() == RepoRequirement::Required {
        if let Err(err) = context.require_repo() {
            render(stderr, context.format(), context.color(), Payload::Error(&err))?;
            return Ok(err.exit_code());
        }
    }
    let mut deprecation = DeprecationContext::new(stderr, !context.color());

    let mut cmd_ctx = CommandCtx::new(&context, &mut deprecation);
    let outcome = match cli.dispatch(&mut cmd_ctx) {
        Ok(o) => o,
        Err(err) => {
            render(stderr, context.format(), context.color(), Payload::Error(&err))?;
            return Ok(err.exit_code());
        }
    };
    render_outcome(outcome, &context, stdout, stderr)
}
fn handle_parse_error(
    err: clap::Error,
    args: &[OsString],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<ExitCode> {
    match err.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
            write!(stdout, "{err}")?;
            Ok(ExitCode::Ok)
        }
        _ => {
            if wants_json_output(args) {
                let mf_error = MfError::usage(err.to_string().trim().to_string(), None);
                render(stderr, output::Format::Json, true, Payload::Error(&mf_error))?;
            } else {
                err.print()?;
            }
            Ok(ExitCode::UsageError)
        }
    }
}
/// Best-effort scan of raw args to decide whether a parse error should still be
/// emitted as a JSON envelope. Clap never parsed these args, so we recognize every
/// accepted form of the JSON selector: `--json`, and `--output`/`-o` set to `json`
/// (space, `=`, and the short attached `-ojson` forms).
fn wants_json_output(args: &[OsString]) -> bool {
    let mut prev_is_output_flag = false;
    for arg in args {
        let Some(arg) = arg.to_str() else {
            prev_is_output_flag = false;
            continue;
        };
        if arg == "--json" {
            return true;
        }
        if prev_is_output_flag && arg == "json" {
            return true;
        }
        if matches!(arg, "--output=json" | "-o=json" | "-ojson") {
            return true;
        }
        prev_is_output_flag = arg == "--output" || arg == "-o";
    }
    false
}

fn render_outcome(
    outcome: CommandOutcome,
    context: &AppContext,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<ExitCode> {
    match outcome {
        CommandOutcome::RootHelp => {
            write_command_help(&mut RootCli::command(), stdout)?;
            Ok(ExitCode::Ok)
        }
        CommandOutcome::GroupHelp(name) => {
            let mut command = RootCli::command();
            let subcommand = command.find_subcommand_mut(name).expect("known subcommand");
            write_command_help(subcommand, stdout)?;
            Ok(ExitCode::Ok)
        }
        CommandOutcome::Completion(shell) => cli::completion::render_completion(shell, stdout),
        CommandOutcome::Success(data, warnings, exit_code) => {
            let data = inject_warnings(data, &warnings);
            render(stdout, context.format(), context.color(), Payload::Success(&data))?;
            Ok(ExitCode::from(exit_code.unwrap_or(0)))
        }
        CommandOutcome::Raw(content, exit_code) => {
            render(stdout, context.format(), context.color(), Payload::Raw(content.as_str()))?;
            Ok(ExitCode::from(exit_code.unwrap_or(0)))
        }
    }
    .or_else(|err| {
        let code = err.exit_code();
        render(stderr, context.format(), context.color(), Payload::Error(&err))?;
        Ok(code)
    })
}
/// Inject collected warnings into the JSON data payload when non-empty.
fn inject_warnings(mut data: serde_json::Value, warnings: &[String]) -> serde_json::Value {
    if !warnings.is_empty() {
        if let serde_json::Value::Object(ref mut map) = data {
            map.insert("warnings".to_string(), serde_json::json!(warnings));
        }
    }
    data
}

fn write_command_help(command: &mut clap::Command, stdout: &mut dyn Write) -> Result<()> {
    let mut buffer = Vec::new();
    command.write_long_help(&mut buffer)?;
    stdout.write_all(&buffer)?;
    Ok(writeln!(stdout)?)
}
