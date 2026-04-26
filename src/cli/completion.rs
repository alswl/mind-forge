use std::io::Write;

use clap::{Args, CommandFactory, ValueEnum};
use clap_complete::{generate, Shell};
use serde::Serialize;

use crate::cli::{CommandOutcome, RootCli};
use crate::error::Result;
use crate::exit::ExitCode;

#[derive(Debug, Clone, Args, Serialize)]
pub struct CompletionArgs {
    #[arg(value_enum)]
    pub shell: ShellKind,
}

#[derive(Debug, Clone, Copy, ValueEnum, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ShellKind {
    Bash,
    Zsh,
    Fish,
    Powershell,
    Elvish,
}

impl ShellKind {
    pub fn into_shell(self) -> Shell {
        match self {
            Self::Bash => Shell::Bash,
            Self::Zsh => Shell::Zsh,
            Self::Fish => Shell::Fish,
            Self::Powershell => Shell::PowerShell,
            Self::Elvish => Shell::Elvish,
        }
    }
}

pub fn dispatch(command: CompletionArgs) -> Result<CommandOutcome> {
    Ok(CommandOutcome::Completion(command.shell.into_shell()))
}

pub fn render_completion(shell: Shell, writer: &mut dyn Write) -> Result<ExitCode> {
    let mut command = RootCli::command();
    generate(shell, &mut command, "mf", writer);
    Ok(ExitCode::Ok)
}
