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
    /// [`MfError::not_implemented_with_hint`].
    #[error("{feature} is not yet implemented")]
    NotImplemented { feature: String, hint: Option<String> },

    /// A requested resource (term, source, article, etc.) was not found.
    /// Construct with [`MfError::not_found`].
    #[error("{message}")]
    NotFound { message: String, hint: Option<String> },

    // ── 023 fix-bugs-2 error kinds ──
    /// A placeholder token in a path template or pattern was not recognised.
    #[error("unknown placeholder '{token}'")]
    UnknownPlaceholder { token: String },

    /// An article has no effective date and cannot be published to a date-templated target.
    #[error("article has no effective date")]
    NoEffectiveDate,

    /// A template pattern contains multiple non-redundant slots (e.g. `{lang}/{date}`).
    #[error("template '{template_name}' has multiple non-redundant slots")]
    MultiSlotTemplate { template_name: String },

    /// A template key does not match `^[a-z][a-z0-9_]*$`.
    #[error("invalid template name '{name}'")]
    InvalidTemplateName { name: String },

    /// A declared article has no article files on disk (FR-005).
    #[error("no article files for '{article}'")]
    NoArticleFiles { article: String, article_path: String },

    /// The build artifact could not be found on disk (renamed from overloaded `not_found`).
    #[error("{message}")]
    BuildArtifactMissing { message: String, hint: Option<String> },

    /// The requested template name is neither a built-in nor an existing path.
    #[error("unknown template '{name}': built-ins are blank, arch, prd, blog; otherwise expected a path under the project root")]
    UnknownTemplate { name: String },

    /// Two H2 headings produced the same slug when splitting a template.
    #[error("duplicate block slug '{slug}' from headings '{h1}' and '{h2}'")]
    DuplicateBlockSlug { slug: String, h1: String, h2: String },

    /// A file/directory shape conflict: --file vs directory or vice versa.
    #[error("cannot create {wanted_shape} '{path}': a {existing_shape} with the same name already exists; remove it manually or pick a different title")]
    ShapeConflict { wanted_shape: String, existing_shape: String, path: PathBuf },
}

impl MfError {
    pub const INIT_REPO_HINT: &str = "Run `mf init` to initialize a new project";

    pub fn usage(message: impl Into<String>, hint: Option<String>) -> Self {
        Self::Usage { message: message.into(), hint }
    }

    pub fn not_in_mind_repo() -> Self {
        Self::NotInMindRepo { hint: Some(Self::INIT_REPO_HINT.to_string()) }
    }

    pub fn file_exists(path: PathBuf) -> Self {
        Self::FileExists { path }
    }

    pub fn not_implemented_with_hint(feature: impl Into<String>, hint: impl Into<String>) -> Self {
        Self::NotImplemented { feature: feature.into(), hint: Some(hint.into()) }
    }

    pub fn not_found(message: impl Into<String>, hint: Option<String>) -> Self {
        Self::NotFound { message: message.into(), hint }
    }

    pub fn build_artifact_missing(message: impl Into<String>, hint: Option<String>) -> Self {
        Self::BuildArtifactMissing { message: message.into(), hint }
    }

    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::Usage { .. } => ExitCode::UsageError,
            Self::NotInMindRepo { .. }
            | Self::IncompatibleSchema { .. }
            | Self::ParseError { .. }
            | Self::FileExists { .. }
            | Self::NotFound { .. }
            | Self::UnknownPlaceholder { .. }
            | Self::NoEffectiveDate
            | Self::MultiSlotTemplate { .. }
            | Self::InvalidTemplateName { .. }
            | Self::NoArticleFiles { .. }
            | Self::BuildArtifactMissing { .. } => ExitCode::Failure,
            Self::NotImplemented { .. } => ExitCode::NotImplemented,
            Self::UnknownTemplate { .. } => ExitCode::UsageError,
            Self::DuplicateBlockSlug { .. } | Self::ShapeConflict { .. } => ExitCode::Failure,
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
            Self::UnknownPlaceholder { .. } => "unknown_placeholder",
            Self::NoEffectiveDate => "no_effective_date",
            Self::MultiSlotTemplate { .. } => "multi_slot_template",
            Self::InvalidTemplateName { .. } => "invalid_template_name",
            Self::NoArticleFiles { .. } => "no_article_files",
            Self::BuildArtifactMissing { .. } => "build_artifact_missing",
            Self::UnknownTemplate { .. } => "unknown_template",
            Self::DuplicateBlockSlug { .. } => "duplicate_block_slug",
            Self::ShapeConflict { .. } => "shape_conflict",
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
            | Self::NotFound { .. }
            | Self::UnknownPlaceholder { .. }
            | Self::MultiSlotTemplate { .. }
            | Self::InvalidTemplateName { .. } => self.to_string(),
            Self::NoEffectiveDate => "article has no effective date".to_string(),
            Self::NoArticleFiles { article, .. } => format!("no article files for '{article}'"),
            Self::BuildArtifactMissing { message, .. } => message.clone(),
            Self::UnknownTemplate { .. } | Self::DuplicateBlockSlug { .. } | Self::ShapeConflict { .. } => {
                self.to_string()
            }
            Self::Internal(error) => error.to_string(),
            Self::Io(error) => error.to_string(),
            Self::Json(error) => error.to_string(),
        }
    }

    pub fn hint(&self) -> Option<&str> {
        match self {
            Self::Usage { hint, .. } => hint.as_deref(),
            Self::NotInMindRepo { hint } => hint.as_deref(),
            Self::IncompatibleSchema { .. } => Some("run `mf upgrade` or update schema_version manually"),
            Self::ParseError { .. } => Some("check the file format and try again"),
            Self::FileExists { .. } => Some("pass --force to overwrite"),
            Self::NotImplemented { hint, .. } => hint.as_deref().or(Some("tracked for future ROADMAP iteration")),
            Self::NotFound { hint, .. } => hint.as_deref(),
            Self::UnknownPlaceholder { .. } => {
                Some("supported placeholders: {date:YYYY}, {date:YYYY-MM}, {date:YYYY-MM-DD}")
            }
            Self::NoEffectiveDate => {
                Some("add a YYYY-MM-DD prefix to the filename, e.g. 'docs/blog/2026-05-15-launch.md'")
            }
            Self::MultiSlotTemplate { .. } => {
                Some("rewrite template to use a single distinguishing slot; coarse-then-fine date nests are accepted")
            }
            Self::InvalidTemplateName { .. } => Some("rename template key to match ^[a-z][a-z0-9_]*$"),
            Self::NoArticleFiles { .. } => Some("create the article file or remove the declaration from mind.yaml"),
            Self::BuildArtifactMissing { hint, .. } => hint.as_deref().or(Some("run `mf build <id>` first")),
            Self::UnknownTemplate { .. } => {
                Some("built-ins are blank, arch, prd, blog; otherwise expected a path under the project root")
            }
            Self::DuplicateBlockSlug { .. } => Some("rename one of the headings to produce a distinct filename"),
            Self::ShapeConflict { .. } => {
                Some("remove the conflicting file or directory manually, or pick a different title")
            }
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
        let err = MfError::not_implemented_with_hint("feature x", "hint");
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
    fn unknown_placeholder_kind_and_hint() {
        let err = MfError::UnknownPlaceholder { token: "{foo}".to_string() };
        assert_eq!(err.kind(), "unknown_placeholder");
        assert!(err.hint().unwrap_or("").contains("supported placeholders"));
    }

    #[test]
    fn no_effective_date_kind_and_hint() {
        let err = MfError::NoEffectiveDate;
        assert_eq!(err.kind(), "no_effective_date");
        assert!(err.hint().unwrap_or("").contains("YYYY-MM-DD"));
    }

    #[test]
    fn multi_slot_template_kind_and_hint() {
        let err = MfError::MultiSlotTemplate { template_name: "test".to_string() };
        assert_eq!(err.kind(), "multi_slot_template");
        assert_eq!(err.exit_code(), ExitCode::Failure);
        assert!(err.hint().unwrap_or("").contains("single distinguishing slot"));
    }

    #[test]
    fn invalid_template_name_kind_and_hint() {
        let err = MfError::InvalidTemplateName { name: "BadName".to_string() };
        assert_eq!(err.kind(), "invalid_template_name");
        assert!(err.hint().unwrap_or("").contains("^[a-z][a-z0-9_]*$"));
    }

    #[test]
    fn no_article_files_kind_and_hint() {
        let err = MfError::NoArticleFiles { article: "test".to_string(), article_path: "docs/test.md".to_string() };
        assert_eq!(err.kind(), "no_article_files");
        assert!(err.hint().unwrap_or("").contains("article file"));
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
    fn unknown_template_kind_and_exit_code() {
        let err = MfError::UnknownTemplate { name: "nope".to_string() };
        assert_eq!(err.kind(), "unknown_template");
        assert_eq!(err.exit_code(), ExitCode::UsageError);
        assert!(err.to_string().contains("nope"));
    }

    #[test]
    fn duplicate_block_slug_kind_and_exit_code() {
        let err = MfError::DuplicateBlockSlug {
            slug: "notes".to_string(),
            h1: "## Notes".to_string(),
            h2: "## NOTES".to_string(),
        };
        assert_eq!(err.kind(), "duplicate_block_slug");
        assert_eq!(err.exit_code(), ExitCode::Failure);
        assert!(err.to_string().contains("notes"));
    }

    #[test]
    fn shape_conflict_kind_and_exit_code() {
        let err = MfError::ShapeConflict {
            wanted_shape: "directory".to_string(),
            existing_shape: "file".to_string(),
            path: PathBuf::from("docs/test.md"),
        };
        assert_eq!(err.kind(), "shape_conflict");
        assert_eq!(err.exit_code(), ExitCode::Failure);
        assert!(err.to_string().contains("directory"));
        assert!(err.to_string().contains("file"));
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
            MfError::not_implemented_with_hint("x", "hint"),
            MfError::not_found("x", Some("hint".to_string())),
            MfError::UnknownPlaceholder { token: "{x}".to_string() },
            MfError::NoEffectiveDate,
            MfError::MultiSlotTemplate { template_name: "t".to_string() },
            MfError::InvalidTemplateName { name: "Bad".to_string() },
            MfError::NoArticleFiles { article: "x".to_string(), article_path: "docs/x.md".to_string() },
            MfError::BuildArtifactMissing { message: "missing".to_string(), hint: None },
            MfError::UnknownTemplate { name: "x".to_string() },
            MfError::DuplicateBlockSlug { slug: "x".to_string(), h1: "a".to_string(), h2: "b".to_string() },
            MfError::ShapeConflict {
                wanted_shape: "dir".to_string(),
                existing_shape: "file".to_string(),
                path: PathBuf::from("/tmp/x"),
            },
        ];
        for err in &cases {
            assert!(err.hint().is_some(), "hint is None for variant: {}", err.kind());
        }
    }
}
