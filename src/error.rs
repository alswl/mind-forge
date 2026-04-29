use std::path::PathBuf;

use anyhow::Error as AnyhowError;
use thiserror::Error;

use crate::exit::ExitCode;

pub type Result<T> = std::result::Result<T, MfError>;

#[derive(Debug, Error)]
pub enum MfError {
    #[error("{message}")]
    Usage { message: String, hint: Option<String> },
    #[error("{0}")]
    Internal(#[from] AnyhowError),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
    #[error("not in a mind repo")]
    NotInMindRepo { hint: Option<String> },
    #[error("incompatible schema: found '{found}', expected one of {expected:?}")]
    IncompatibleSchema { path: PathBuf, found: String, expected: Vec<String> },
    #[error("{kind} parse error in {path}: {detail}")]
    ParseError { kind: String, path: PathBuf, detail: String },
    #[error("refusing to overwrite existing file: {path}")]
    FileExists { path: PathBuf },
    #[error("{feature} is not yet implemented")]
    NotImplemented { feature: String },
}

impl MfError {
    pub const INIT_REPO_HINT: &str =
        "Run 'mf config init --target project' to initialize a new project";

    pub fn usage(message: impl Into<String>, hint: Option<String>) -> Self {
        Self::Usage { message: message.into(), hint }
    }

    pub fn not_in_mind_repo() -> Self {
        Self::NotInMindRepo { hint: Some(Self::INIT_REPO_HINT.to_string()) }
    }

    pub fn file_exists(path: PathBuf) -> Self {
        Self::FileExists { path }
    }

    pub fn not_implemented(feature: impl Into<String>) -> Self {
        Self::NotImplemented { feature: feature.into() }
    }

    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::Usage { .. } => ExitCode::UsageError,
            Self::NotInMindRepo { .. }
            | Self::IncompatibleSchema { .. }
            | Self::ParseError { .. }
            | Self::FileExists { .. } => ExitCode::Failure,
            Self::NotImplemented { .. } => ExitCode::NotImplemented,
            Self::Internal(_) | Self::Io(_) | Self::Json(_) => ExitCode::Failure,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::Usage { .. } => "usage",
            Self::NotInMindRepo { .. } => "not-in-mind-repo",
            Self::IncompatibleSchema { .. } => "incompatible-schema",
            Self::ParseError { .. } => "parse-error",
            Self::FileExists { .. } => "file-exists",
            Self::NotImplemented { .. } => "not-implemented",
            Self::Internal(_) | Self::Io(_) | Self::Json(_) => "internal",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::Usage { message, .. } => message.clone(),
            Self::NotInMindRepo { .. }
            | Self::IncompatibleSchema { .. }
            | Self::ParseError { .. }
            | Self::FileExists { .. }
            | Self::NotImplemented { .. } => self.to_string(),
            Self::Internal(error) => error.to_string(),
            Self::Io(error) => error.to_string(),
            Self::Json(error) => error.to_string(),
        }
    }

    pub fn hint(&self) -> Option<&str> {
        match self {
            Self::Usage { hint, .. } => hint.as_deref(),
            Self::NotInMindRepo { hint } => hint.as_deref(),
            Self::IncompatibleSchema { .. } => {
                Some("run 'mf upgrade' or update schema_version manually")
            }
            Self::ParseError { .. } => Some("check the file format and try again"),
            Self::FileExists { .. } => Some("pass --force to overwrite"),
            Self::NotImplemented { .. } => Some("tracked for future ROADMAP iteration"),
            _ => None,
        }
    }
}
