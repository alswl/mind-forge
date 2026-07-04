pub mod capability;
pub mod confirm;
pub mod link;
pub mod list;
pub mod show;
pub mod verb;
pub mod warning;

use std::io::Write;

use serde::Serialize;

use crate::error::{MfError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Text,
    Json,
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

pub enum Payload<'a> {
    Success(&'a serde_json::Value),
    Raw(&'a str),
    Error(&'a MfError),
}

pub fn render(writer: &mut dyn Write, format: Format, color: bool, payload: Payload<'_>) -> Result<()> {
    match payload {
        Payload::Success(data) => render_success_inner(writer, format, color, data),
        Payload::Raw(content) => render_raw_inner(writer, format, color, content),
        Payload::Error(error) => render_error_inner(writer, format, color, error),
    }
}

pub(super) fn render_success_inner(
    writer: &mut dyn Write,
    format: Format,
    _color: bool,
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
            let envelope = serde_json::json!({ "status": "ok", "command": "mf", "data": data });
            serde_json::to_writer(&mut *writer, &envelope)?;
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
pub(super) fn render_raw_inner(writer: &mut dyn Write, format: Format, _color: bool, content: &str) -> Result<()> {
    match format {
        Format::Text => {
            writeln!(writer, "{content}")?;
        }
        Format::Json => {
            let data: serde_json::Value =
                serde_json::from_str(content).unwrap_or(serde_json::Value::String(content.to_string()));
            let envelope = serde_json::json!({ "status": "ok", "command": "mf", "data": data });
            serde_json::to_writer(&mut *writer, &envelope)?;
            writeln!(writer)?;
        }
    }
    Ok(())
}

pub(super) fn render_error_inner(writer: &mut dyn Write, format: Format, _color: bool, error: &MfError) -> Result<()> {
    let message = error.message();
    match format {
        Format::Text => {
            writeln!(writer, "error: {message}")?;
            if let Some(hint) = error.hint() {
                writeln!(writer)?;
                writeln!(writer, "Hint: {hint}")?;
            }
            writeln!(writer)?;
            writeln!(writer, "Run `mf --help` for usage.")?;
        }
        Format::Json => {
            let envelope = ErrorEnvelope {
                status: "error",
                command: "mf",
                error: ErrorDetail { kind: error.kind(), message: &message, hint: error.hint() },
            };
            serde_json::to_writer(&mut *writer, &envelope)?;
            writeln!(writer)?;
        }
    }
    Ok(())
}
