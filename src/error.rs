use std::path::PathBuf;

use anyhow::Error as AnyhowError;
use thiserror::Error;

use crate::exit::ExitCode;

pub type Result<T> = std::result::Result<T, MfError>;

#[derive(Debug, Error)]
pub enum MfError {
    /// User-provided input is invalid. Construct with [`MfError::usage`].
    /// Do NOT use for I/O, parsing, or serialization failures.
    #[error("{message}")]
    Usage { message: String, hint: Option<String> },

    /// Catch-all for unexpected internal failures (e.g. serialization,
    /// invariant violations). Wraps an [`anyhow::Error`]. Construct via
    /// `MfError::Internal(anyhow::anyhow!(...))` or with `anyhow::Error::from`.
    /// Do NOT use for user-facing errors.
    #[error("{0}")]
    Internal(#[from] AnyhowError),

    /// I/O operation failed (file read/write, filesystem metadata, etc.).
    /// Automatically constructed via `?` from [`std::io::Error`].
    /// Do NOT use for YAML/JSON parse errors or user input validation.
    #[error("{0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization or deserialization failed. Automatically
    /// constructed via `?` from [`serde_json::Error`].
    #[error("{0}")]
    Json(#[from] serde_json::Error),

    /// The current directory is not inside a mind repository.
    #[error("not in a mind repo")]
    NotInMindRepo { hint: Option<String> },

    /// The schema version in mind-index.yaml is not supported by this
    /// version of the CLI.
    #[error("incompatible schema: found '{found}', expected one of {expected:?}")]
    IncompatibleSchema { path: PathBuf, found: String, expected: Vec<String> },

    /// A YAML/JSON/mind file could not be parsed. Includes file path
    /// and parser detail.
    #[error("{kind} parse error in {path}: {detail}")]
    ParseError { kind: String, path: PathBuf, detail: String },

    /// Refusing to overwrite an existing file without --force.
    #[error("refusing to overwrite existing file: {path}")]
    FileExists { path: PathBuf },

    /// A feature is not yet implemented. Construct with
    /// [`MfError::not_implemented`] or [`MfError::not_implemented_with_hint`].
    #[error("{feature} is not yet implemented")]
    NotImplemented { feature: String, hint: Option<String> },

    /// A requested resource (term, source, article, etc.) was not found.
    /// Construct with [`MfError::not_found`].
    #[error("{message}")]
    NotFound { message: String, hint: Option<String> },
}

impl MfError {
    pub const INIT_REPO_HINT: &str = "Run 'mf config init --target project' to initialize a new project";

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
        Self::NotImplemented { feature: feature.into(), hint: None }
    }

    pub fn not_implemented_with_hint(feature: impl Into<String>, hint: impl Into<String>) -> Self {
        Self::NotImplemented { feature: feature.into(), hint: Some(hint.into()) }
    }

    pub fn not_found(message: impl Into<String>, hint: Option<String>) -> Self {
        Self::NotFound { message: message.into(), hint }
    }

    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::Usage { .. } => ExitCode::UsageError,
            Self::NotInMindRepo { .. }
            | Self::IncompatibleSchema { .. }
            | Self::ParseError { .. }
            | Self::FileExists { .. }
            | Self::NotFound { .. } => ExitCode::Failure,
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
            Self::NotFound { .. } => "not-found",
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
            | Self::NotImplemented { .. }
            | Self::NotFound { .. } => self.to_string(),
            Self::Internal(error) => error.to_string(),
            Self::Io(error) => error.to_string(),
            Self::Json(error) => error.to_string(),
        }
    }

    pub fn hint(&self) -> Option<&str> {
        match self {
            Self::Usage { hint, .. } => hint.as_deref(),
            Self::NotInMindRepo { hint } => hint.as_deref(),
            Self::IncompatibleSchema { .. } => Some("run 'mf upgrade' or update schema_version manually"),
            Self::ParseError { .. } => Some("check the file format and try again"),
            Self::FileExists { .. } => Some("pass --force to overwrite"),
            Self::NotImplemented { hint, .. } => hint.as_deref().or(Some("tracked for future ROADMAP iteration")),
            Self::NotFound { hint, .. } => hint.as_deref(),
            Self::Internal(_) | Self::Io(_) | Self::Json(_) => Some("this is an internal error; please report it"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_kind_is_usage() {
        let err = MfError::usage("bad input", None::<String>);
        assert_eq!(err.kind(), "usage");
    }

    #[test]
    fn io_kind_is_io() {
        let err = MfError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "file not found"));
        assert_eq!(err.kind(), "internal");
    }

    #[test]
    fn internal_kind_is_internal() {
        let err = MfError::Internal(anyhow::anyhow!("unexpected error"));
        assert_eq!(err.kind(), "internal");
    }

    #[test]
    fn json_kind_is_internal() {
        let err = MfError::Json(serde_json::from_str::<serde_json::Value>("").unwrap_err());
        assert_eq!(err.kind(), "internal");
    }

    #[test]
    fn not_in_mind_repo_kind_is_not_in_mind_repo() {
        let err = MfError::not_in_mind_repo();
        assert_eq!(err.kind(), "not-in-mind-repo");
    }

    #[test]
    fn file_exists_kind_is_file_exists() {
        let err = MfError::file_exists(PathBuf::from("/tmp/test"));
        assert_eq!(err.kind(), "file-exists");
    }

    #[test]
    fn not_found_kind_is_not_found() {
        let err = MfError::not_found("missing", None::<String>);
        assert_eq!(err.kind(), "not-found");
    }

    #[test]
    fn not_implemented_kind_is_not_implemented() {
        let err = MfError::not_implemented("feature x");
        assert_eq!(err.kind(), "not-implemented");
    }

    #[test]
    fn parse_error_kind_is_parse_error() {
        let err = MfError::ParseError {
            kind: "yaml".to_string(),
            path: PathBuf::from("/tmp/test.yaml"),
            detail: "syntax error".to_string(),
        };
        assert_eq!(err.kind(), "parse-error");
    }

    #[test]
    fn incompatible_schema_kind_is_incompatible_schema() {
        let err = MfError::IncompatibleSchema {
            path: PathBuf::from("/tmp/index.yaml"),
            found: "2".to_string(),
            expected: vec!["1".to_string()],
        };
        assert_eq!(err.kind(), "incompatible-schema");
    }

    // ── hint tests (US9 / T066) ──

    #[test]
    fn internal_hint_is_some() {
        let err = MfError::Internal(anyhow::anyhow!("test"));
        assert!(err.hint().is_some());
    }

    #[test]
    fn io_hint_is_some() {
        let err = MfError::Io(std::io::Error::other("test"));
        assert!(err.hint().is_some());
    }

    #[test]
    fn json_hint_is_some() {
        let err = MfError::Json(serde_json::from_str::<serde_json::Value>("").unwrap_err());
        assert!(err.hint().is_some());
    }

    #[test]
    fn all_variants_hint_returns_some() {
        let cases: Vec<MfError> = vec![
            MfError::usage("test", Some("hint".to_string())),
            MfError::Internal(anyhow::anyhow!("test")),
            MfError::Io(std::io::Error::other("test")),
            MfError::Json(serde_json::from_str::<serde_json::Value>("").unwrap_err()),
            MfError::not_in_mind_repo(),
            MfError::IncompatibleSchema {
                path: PathBuf::from("/tmp/x.yaml"),
                found: "2".to_string(),
                expected: vec!["1".to_string()],
            },
            MfError::ParseError {
                kind: "yaml".to_string(),
                path: PathBuf::from("/tmp/x.yaml"),
                detail: "syntax".to_string(),
            },
            MfError::file_exists(PathBuf::from("/tmp/x")),
            MfError::not_implemented("x"),
            MfError::not_found("x", Some("hint".to_string())),
        ];
        for err in &cases {
            assert!(err.hint().is_some(), "hint is None for variant: {}", err.kind());
        }
    }
}
