use std::path::Path;

use serde::Serialize;

use crate::service::publisher::PublisherDiagnosticKind;

#[derive(Debug, Clone, Serialize)]
pub struct PublishersOutcome {
    pub publishers: Vec<PublisherView>,
    pub diagnostics: Vec<PublisherDiagnosticView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PublisherView {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub target_type: String,
    pub enabled: bool,
    pub source_path: String,
    pub status: String,
    pub required_inputs: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PublisherDiagnosticView {
    pub kind: String,
    pub path: Option<String>,
    pub publisher_name: Option<String>,
    pub message: String,
    pub hint: Option<String>,
}

impl PublishersOutcome {
    pub fn from_report(report: &crate::service::publisher::PublisherDiscoveryReport, repo_root: &Path) -> Self {
        let publishers = report
            .publishers
            .iter()
            .map(|p| {
                let type_str = target_type_str(&p.target.target_type);
                let status_str = match p.status {
                    crate::service::publisher::PublisherStatus::Available => "available",
                    crate::service::publisher::PublisherStatus::Disabled => "disabled",
                };
                PublisherView {
                    name: p.name.clone(),
                    label: p.label.clone(),
                    description: p.description.clone(),
                    target_type: type_str,
                    enabled: p.target.enabled,
                    source_path: p.source_path.clone(),
                    status: status_str.to_string(),
                    required_inputs: p.required_inputs.clone(),
                }
            })
            .collect();

        let diagnostics = report
            .diagnostics
            .iter()
            .map(|d| {
                let kind = diagnostic_kind_str(&d.kind);
                let path = d.path.as_ref().map(|p| crate::service::util::repo_relative_path(repo_root, p));
                PublisherDiagnosticView {
                    kind,
                    path,
                    publisher_name: d.publisher_name.clone(),
                    message: d.message.clone(),
                    hint: d.hint.clone(),
                }
            })
            .collect();

        Self { publishers, diagnostics }
    }
}

fn target_type_str(t: &crate::model::config::PublishTargetType) -> String {
    match t {
        crate::model::config::PublishTargetType::Local => "local".to_string(),
        crate::model::config::PublishTargetType::YuquePrompt => "yuque-prompt".to_string(),
        crate::model::config::PublishTargetType::Yuque => "yuque".to_string(),
        crate::model::config::PublishTargetType::GithubPages => "github_pages".to_string(),
        crate::model::config::PublishTargetType::Custom => "custom".to_string(),
        crate::model::config::PublishTargetType::YuqueCc => "yuque_cc".to_string(),
    }
}

fn diagnostic_kind_str(kind: &PublisherDiagnosticKind) -> String {
    match kind {
        PublisherDiagnosticKind::MalformedYaml => "malformed_yaml".to_string(),
        PublisherDiagnosticKind::MissingRequiredField => "missing_required_field".to_string(),
        PublisherDiagnosticKind::InvalidName => "invalid_name".to_string(),
        PublisherDiagnosticKind::ReservedName => "reserved_name".to_string(),
        PublisherDiagnosticKind::DuplicateName => "duplicate_name".to_string(),
        PublisherDiagnosticKind::SecretField => "secret_field".to_string(),
    }
}
