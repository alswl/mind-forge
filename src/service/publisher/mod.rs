pub mod definition;
pub mod discovery;
pub mod validation;

use std::path::Path;

use crate::error::Result;

pub struct Publisher {
    pub name: String,
    pub label: Option<String>,
    pub description: Option<String>,
    pub target: crate::model::config::PublishTarget,
    pub source_path: String,
    pub required_inputs: Vec<String>,
    pub status: PublisherStatus,
}

pub enum PublisherStatus {
    Available,
    Disabled,
}

pub struct PublisherDiscoveryReport {
    pub publishers: Vec<Publisher>,
    pub diagnostics: Vec<PublisherDiagnostic>,
}

pub struct PublisherDiagnostic {
    pub kind: PublisherDiagnosticKind,
    pub path: Option<std::path::PathBuf>,
    pub publisher_name: Option<String>,
    pub message: String,
    pub hint: Option<String>,
}

#[derive(Debug)]
pub enum PublisherDiagnosticKind {
    MalformedYaml,
    MissingRequiredField,
    InvalidName,
    ReservedName,
    DuplicateName,
    SecretField,
}

pub struct ResolvedPublisherTarget {
    pub target: crate::model::config::PublishTarget,
}

pub fn discover(repo_root: &Path) -> Result<PublisherDiscoveryReport> {
    discovery::scan(repo_root)
}

pub fn resolve_target(
    repo_root: &Path,
    name: &str,
    config: &crate::model::config::MindConfig,
) -> Result<ResolvedPublisherTarget> {
    validation::resolve(repo_root, name, config)
}
