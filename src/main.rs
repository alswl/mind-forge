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
use cli::{CommandOutcome, RepoRequirement, RootCli};
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
    // Bridge the --no-color flag to the NO_COLOR convention so every renderer
    // (which keys off NO_COLOR) honors it uniformly.
    if cli.global.no_color {
        std::env::set_var("NO_COLOR", "1");
    }
    let context = match AppContext::from_global_opts(&cli.global) {
        Ok(context) => context,
        Err(err) => {
            render(stderr, cli.global.format, Payload::Error(&err))?;
            return Ok(err.exit_code());
        }
    };
    tracing::debug!(config_path = %context.config_path.display(), "resolved config path");
    if cli.requires_repo() == RepoRequirement::Required {
        if let Err(err) = context.require_repo() {
            render(stderr, context.format, Payload::Error(&err))?;
            return Ok(err.exit_code());
        }
    }
    let mut deprecation = DeprecationContext::new(stderr, cli.global.no_color);

    // Handle --install-completion / --show-completion with deprecation warnings
    if let Some(shell) = cli.global.install_completion {
        deprecation.warn_subject("--install-completion", "mf completion");
        return cli::completion::render_completion(shell.into_shell(), stdout);
    }
    if let Some(shell) = cli.global.show_completion {
        deprecation.warn_subject("--show-completion", "mf completion");
        return cli::completion::render_completion(shell.into_shell(), stdout);
    }

    let outcome = match cli.dispatch(context.repo_root.as_ref(), context.format, &mut deprecation) {
        Ok(o) => o,
        Err(err) => {
            render(stderr, context.format, Payload::Error(&err))?;
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
            if args.windows(2).any(|w| w[0] == "--format" && w[1] == "json") {
                let mf_error = MfError::usage(err.to_string().trim().to_string(), None);
                render(stderr, output::Format::Json, Payload::Error(&mf_error))?;
            } else {
                err.print()?;
            }
            Ok(ExitCode::UsageError)
        }
    }
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
        CommandOutcome::Success(data, exit_code) => {
            render(stdout, context.format, Payload::Success(&data))?;
            Ok(ExitCode::from(exit_code.unwrap_or(0)))
        }
        CommandOutcome::Raw(content, exit_code) => {
            render(stdout, context.format, Payload::Raw(content.as_str()))?;
            Ok(ExitCode::from(exit_code.unwrap_or(0)))
        }
    }
    .or_else(|err| {
        let code = err.exit_code();
        render(stderr, context.format, Payload::Error(&err))?;
        Ok(code)
    })
}
fn write_command_help(command: &mut clap::Command, stdout: &mut dyn Write) -> Result<()> {
    let mut buffer = Vec::new();
    command.write_long_help(&mut buffer)?;
    stdout.write_all(&buffer)?;
    Ok(writeln!(stdout)?)
}
