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
}

impl MfError {
    pub fn usage(message: impl Into<String>, hint: Option<String>) -> Self {
        Self::Usage { message: message.into(), hint }
    }

    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::Usage { .. } => ExitCode::UsageError,
            Self::Internal(_) | Self::Io(_) | Self::Json(_) => ExitCode::Failure,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::Usage { .. } => "usage",
            Self::Internal(_) | Self::Io(_) | Self::Json(_) => "internal",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::Usage { message, .. } => message.clone(),
            Self::Internal(error) => error.to_string(),
            Self::Io(error) => error.to_string(),
            Self::Json(error) => error.to_string(),
        }
    }

    pub fn hint(&self) -> Option<&str> {
        match self {
            Self::Usage { hint, .. } => hint.as_deref(),
            _ => None,
        }
    }
}
