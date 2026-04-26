pub mod placeholder;

use std::io::Write;

use serde::Serialize;

use crate::error::{MfError, Result};

pub use placeholder::PlaceholderInvocation;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Text,
    Json,
}

impl Format {
    pub fn is_json(self) -> bool {
        matches!(self, Self::Json)
    }
}

#[derive(Debug, Serialize)]
struct ErrorEnvelope<'a> {
    status: &'static str,
    command: &'a str,
    error: ErrorDetail<'a>,
}

#[derive(Debug, Serialize)]
struct ErrorDetail<'a> {
    kind: &'a str,
    message: &'a str,
    hint: Option<&'a str>,
}

pub fn render_placeholder(
    writer: &mut dyn Write,
    format: Format,
    invocation: &PlaceholderInvocation,
    _color_enabled: bool,
) -> Result<()> {
    match format {
        Format::Text => {
            writeln!(writer, "[not implemented] {}", invocation.command)?;
            writeln!(writer, "  args: {}", invocation.args_text())?;
            writeln!(
                writer,
                "  This command is a framework placeholder; implementation will follow."
            )?;
        }
        Format::Json => {
            serde_json::to_writer_pretty(&mut *writer, &invocation.to_json())?;
            writeln!(writer)?;
        }
    }
    Ok(())
}

pub fn render_error(writer: &mut dyn Write, format: Format, error: &MfError) -> Result<()> {
    let message = error.message();
    match format {
        Format::Text => {
            writeln!(writer, "error: {message}")?;
            if let Some(hint) = error.hint() {
                writeln!(writer)?;
                writeln!(writer, "Hint: {hint}")?;
            }
            writeln!(writer)?;
            writeln!(writer, "Run 'mf --help' for usage.")?;
        }
        Format::Json => {
            let envelope = ErrorEnvelope {
                status: "error",
                command: "mf",
                error: ErrorDetail { kind: error.kind(), message: &message, hint: error.hint() },
            };
            serde_json::to_writer_pretty(&mut *writer, &envelope)?;
            writeln!(writer)?;
        }
    }
    Ok(())
}
