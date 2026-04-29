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

pub fn render_success(
    writer: &mut dyn Write,
    format: Format,
    data: &serde_json::Value,
) -> Result<()> {
    match format {
        Format::Text => match data {
            serde_json::Value::String(s) => writeln!(writer, "{s}")?,
            serde_json::Value::Object(map) => {
                let max_key = map.keys().map(|k| k.len()).max().unwrap_or(0);
                for (key, value) in map {
                    let val_str = match value {
                        serde_json::Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    writeln!(writer, "{key:>max_key$}  {val_str}")?;
                }
            }
            other => writeln!(writer, "{other}")?,
        },
        Format::Json => {
            let envelope = serde_json::json!({ "status": "ok", "data": data });
            serde_json::to_writer_pretty(&mut *writer, &envelope)?;
            writeln!(writer)?;
        }
    }
    Ok(())
}

/// Render a pre-serialized raw string.
///
/// In text mode the string is printed as-is.
/// In JSON mode the string is parsed as JSON (if valid) and embedded
/// directly into the `{ status, data }` envelope, avoiding double-encoding.
pub fn render_raw(writer: &mut dyn Write, format: Format, content: &str) -> Result<()> {
    match format {
        Format::Text => {
            writeln!(writer, "{content}")?;
        }
        Format::Json => {
            let data: serde_json::Value = serde_json::from_str(content)
                .unwrap_or(serde_json::Value::String(content.to_string()));
            let envelope = serde_json::json!({ "status": "ok", "data": data });
            serde_json::to_writer_pretty(&mut *writer, &envelope)?;
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
