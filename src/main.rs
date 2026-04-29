mod cli;
mod error;
mod exit;
mod model;
mod output;
mod runtime;
mod service;

use std::ffi::OsString;
use std::io::{self, Write};
use std::process::ExitCode as ProcessExitCode;

use clap::{CommandFactory, Parser};
use cli::{CommandOutcome, HelpTarget, RootCli};
use error::{MfError, Result};
use exit::ExitCode;
use output::{render_error, render_placeholder, render_raw, render_success};
use runtime::AppContext;

fn main() -> ProcessExitCode {
    let args: Vec<OsString> = std::env::args_os().collect();
    match run(args, &mut io::stdout(), &mut io::stderr()) {
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

    let wants_json = args.windows(2).any(|w| w[0] == "--format" && w[1] == "json");

    let cli = match RootCli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(err) => {
            return handle_parse_error(err, wants_json, stdout, stderr);
        }
    };

    let context = match AppContext::from_global_opts(&cli.global) {
        Ok(context) => context,
        Err(err) => {
            render_error(stderr, cli.global.format, &err)?;
            return Ok(err.exit_code());
        }
    };
    tracing::debug!(config_path = %context.config_path.display(), "resolved config path");

    // Mind Repo 守卫检查：需要上下文的命令必须在 Mind Repo 内运行
    if cli.command_needs_repo() {
        if let Err(err) = context.require_repo() {
            render_error(stderr, context.format, &err)?;
            return Ok(err.exit_code());
        }
    }

    let outcome = match cli.dispatch(context.repo_root.as_ref(), context.format) {
        Ok(outcome) => outcome,
        Err(err) => {
            let code = err.exit_code();
            render_error(stderr, context.format, &err)?;
            return Ok(code);
        }
    };
    match outcome {
        CommandOutcome::RootHelp => {
            write_command_help(&mut RootCli::command(), stdout)?;
            Ok(ExitCode::Ok)
        }
        CommandOutcome::GroupHelp(target) => {
            write_group_help(target, stdout)?;
            Ok(ExitCode::Ok)
        }
        CommandOutcome::Completion(shell) => cli::completion::render_completion(shell, stdout),
        CommandOutcome::Success(data) => {
            render_success(stdout, context.format, &data)?;
            Ok(ExitCode::Ok)
        }
        CommandOutcome::Placeholder(invocation) => {
            render_placeholder(stdout, context.format, &invocation, context.color_enabled)?;
            Ok(ExitCode::NotImplemented)
        }
        CommandOutcome::Raw(content) => {
            render_raw(stdout, context.format, &content)?;
            Ok(ExitCode::Ok)
        }
    }
    .or_else(|err| {
        let code = err.exit_code();
        render_error(stderr, context.format, &err)?;
        Ok(code)
    })
}

fn handle_parse_error(
    err: clap::Error,
    wants_json: bool,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<ExitCode> {
    use clap::error::ErrorKind;

    match err.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
            write!(stdout, "{err}")?;
            Ok(ExitCode::Ok)
        }
        _ => {
            if wants_json {
                let message = err.to_string();
                let mf_error = MfError::usage(message.trim().to_string(), None);
                render_error(stderr, output::Format::Json, &mf_error)?;
            } else {
                err.print()?;
            }
            Ok(ExitCode::UsageError)
        }
    }
}

fn write_group_help(target: HelpTarget, stdout: &mut dyn Write) -> Result<()> {
    let mut command = RootCli::command();
    let subcommand = match target {
        HelpTarget::Source => command.find_subcommand_mut("source").expect("source command exists"),
        HelpTarget::Asset => command.find_subcommand_mut("asset").expect("asset command exists"),
        HelpTarget::Project => {
            command.find_subcommand_mut("project").expect("project command exists")
        }
        HelpTarget::Article => {
            command.find_subcommand_mut("article").expect("article command exists")
        }
        HelpTarget::Term => command.find_subcommand_mut("term").expect("term command exists"),
        HelpTarget::Config => command.find_subcommand_mut("config").expect("config command exists"),
        HelpTarget::Publish => {
            command.find_subcommand_mut("publish").expect("publish command exists")
        }
    };
    write_command_help(subcommand, stdout)?;
    Ok(())
}

fn write_command_help(command: &mut clap::Command, stdout: &mut dyn Write) -> Result<()> {
    let mut buffer = Vec::new();
    command.write_long_help(&mut buffer)?;
    stdout.write_all(&buffer)?;
    writeln!(stdout)?;
    Ok(())
}
