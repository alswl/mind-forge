use std::path::Path;

use crate::error::{MfError, Result};

use super::{Publisher, PublisherDiagnostic, PublisherDiagnosticKind, PublisherStatus, ResolvedPublisherTarget};

const RESERVED_NAMES: &[&str] = &["default", "all", "none"];
const SECRET_KEYS: &[&str] = &["token", "password", "secret", "credential", "api_key", "access_key"];

/// Validate that a publisher name is valid kebab-case.
pub fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(MfError::usage(
            "publisher name cannot be empty",
            Some("provide a non-empty name in the definition file or use the filename stem".to_string()),
        ));
    }
    if RESERVED_NAMES.contains(&name) {
        return Err(MfError::usage(
            format!("publisher name '{name}' is reserved"),
            Some(format!("choose a different name; reserved names: {}", RESERVED_NAMES.join(", "))),
        ));
    }
    if !name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        return Err(MfError::usage(
            format!("publisher name '{name}' is not valid kebab-case"),
            Some("use only lowercase ASCII letters, digits, and hyphens".to_string()),
        ));
    }
    if name.starts_with('-') || name.ends_with('-') {
        return Err(MfError::usage(
            format!("publisher name '{name}' cannot start or end with a hyphen"),
            Some("remove leading or trailing hyphens from the name".to_string()),
        ));
    }
    Ok(())
}

/// Check if a string is a secret-bearing key.
pub fn is_secret_key(key: &str) -> bool {
    SECRET_KEYS.contains(&key)
}

/// Recursively check if a JSON value contains any secret-bearing keys.
pub fn contains_secret_key(value: &serde_json::Value, path_prefix: &str) -> Option<String> {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                let full_path = if path_prefix.is_empty() { k.clone() } else { format!("{path_prefix}.{k}") };
                if is_secret_key(k) {
                    return Some(full_path);
                }
                if let Some(found) = contains_secret_key(v, &full_path) {
                    return Some(found);
                }
            }
            None
        }
        _ => None,
    }
}

/// Process a single definition file: parse, validate, and either add a valid
/// Publisher or collect diagnostics.
pub fn process_definition(
    content: &str,
    stem: &str,
    repo_rel: &str,
    path: &Path,
    publishers: &mut Vec<Publisher>,
    diagnostics: &mut Vec<PublisherDiagnostic>,
) {
    let def: super::definition::PublisherDefinition = match serde_yaml::from_str(content) {
        Ok(d) => d,
        Err(e) => {
            diagnostics.push(PublisherDiagnostic {
                kind: PublisherDiagnosticKind::MalformedYaml,
                path: Some(path.to_path_buf()),
                publisher_name: None,
                message: format!("publisher definition is not valid YAML: {e}"),
                hint: Some(format!("fix {repo_rel} or remove the file")),
            });
            return;
        }
    };

    let name = def.name.clone().unwrap_or_else(|| stem.to_string());

    let mut local_diagnostics: Vec<PublisherDiagnostic> = Vec::new();

    // Validate name
    if let Err(e) = validate_name(&name) {
        let kind = if RESERVED_NAMES.contains(&name.as_str()) {
            PublisherDiagnosticKind::ReservedName
        } else {
            PublisherDiagnosticKind::InvalidName
        };
        local_diagnostics.push(PublisherDiagnostic {
            kind,
            path: Some(path.to_path_buf()),
            publisher_name: Some(name.clone()),
            message: e.message(),
            hint: e.hint().map(|s| s.to_string()),
        });
    }

    // Validate type
    let target_type = match def.target_type {
        Some(t) => t,
        None => {
            local_diagnostics.push(PublisherDiagnostic {
                kind: PublisherDiagnosticKind::MissingRequiredField,
                path: Some(path.to_path_buf()),
                publisher_name: Some(name.clone()),
                message: "publisher definition is missing required field: type".to_string(),
                hint: Some(format!("add `type: local` or another valid type to {repo_rel}")),
            });
            diagnostics.extend(local_diagnostics);
            return; // can't proceed without type
        }
    };

    // Validate local type has config.path
    if matches!(target_type, crate::model::config::PublishTargetType::Local) {
        let has_path = def
            .config
            .as_ref()
            .and_then(|c| c.get("path"))
            .and_then(|v| v.as_str())
            .map(|s| !s.is_empty())
            .unwrap_or(false);
        if !has_path {
            local_diagnostics.push(PublisherDiagnostic {
                kind: PublisherDiagnosticKind::MissingRequiredField,
                path: Some(path.to_path_buf()),
                publisher_name: Some(name.clone()),
                message: "local publisher definition is missing required field: config.path".to_string(),
                hint: Some(format!("add `config.path: <directory>` to {repo_rel}")),
            });
        }
    }

    // Check for secret-bearing keys
    if let Some(ref config) = def.config {
        if let Some(found) = contains_secret_key(config, "config") {
            local_diagnostics.push(PublisherDiagnostic {
                kind: PublisherDiagnosticKind::SecretField,
                path: Some(path.to_path_buf()),
                publisher_name: Some(name.clone()),
                message: format!("publisher definition contains a secret-bearing field: {found}"),
                hint: Some(
                    "move secret values to environment variables or explicit CLI parameters and declare required_inputs instead".to_string(),
                ),
            });
        }
    }

    // Check for duplicates
    let is_duplicate = publishers.iter().any(|p| p.name == name);

    // If name is valid and not duplicate and no secret field issues, add publisher
    let has_name_error = local_diagnostics
        .iter()
        .any(|d| matches!(d.kind, PublisherDiagnosticKind::InvalidName | PublisherDiagnosticKind::ReservedName));
    let has_secret_error = local_diagnostics.iter().any(|d| matches!(d.kind, PublisherDiagnosticKind::SecretField));

    if !has_name_error && !is_duplicate && !has_secret_error {
        let status = if def.enabled { PublisherStatus::Available } else { PublisherStatus::Disabled };

        let target = crate::model::config::PublishTarget {
            name: name.clone(),
            target_type,
            enabled: def.enabled,
            config: def.config.clone(),
            path: None,
            prefix: None,
            book_slug: None,
            namespace: None,
        };

        publishers.push(Publisher {
            name: name.clone(),
            label: def.label,
            description: def.description,
            target,
            source_path: repo_rel.to_string(),
            required_inputs: def.required_inputs,
            status,
        });
    }

    if is_duplicate {
        diagnostics.push(PublisherDiagnostic {
            kind: PublisherDiagnosticKind::DuplicateName,
            path: Some(path.to_path_buf()),
            publisher_name: Some(name),
            message: format!("duplicate publisher name in {}", repo_rel),
            hint: Some("use a unique publisher name or remove the duplicate definition file".to_string()),
        });
    }

    diagnostics.extend(local_diagnostics);
}

/// Resolve a target by name: file-based publisher first, then mind.yaml fallback.
pub fn resolve(
    repo_root: &Path,
    name: &str,
    config: &crate::model::config::MindConfig,
) -> Result<ResolvedPublisherTarget> {
    let report = super::discovery::scan(repo_root)?;

    // File-based publisher found → validate status
    if let Some(pub_entry) = report.publishers.iter().find(|p| p.name == name) {
        // Even if found in the list, a blocking diagnostic may exist (e.g. secret field
        // blocks listing but the publisher was registered before the check was added).
        let has_blocking = report.diagnostics.iter().any(|d| {
            d.publisher_name.as_deref() == Some(name)
                && matches!(
                    d.kind,
                    PublisherDiagnosticKind::InvalidName
                        | PublisherDiagnosticKind::ReservedName
                        | PublisherDiagnosticKind::DuplicateName
                        | PublisherDiagnosticKind::SecretField
                )
        });
        if has_blocking {
            return Err(MfError::usage(
                format!("publisher '{name}' has configuration errors and cannot be used"),
                Some("run `mf publish target list` for details".to_string()),
            ));
        }
        match pub_entry.status {
            PublisherStatus::Disabled => {
                return Err(MfError::usage(
                    format!("publisher '{name}' is disabled"),
                    Some("set `enabled: true` on the publisher definition".to_string()),
                ));
            }
            PublisherStatus::Available => {
                return Ok(ResolvedPublisherTarget { target: pub_entry.target.clone() });
            }
        }
    }

    // Publisher not in the list; any diagnostic for this name means it was excluded
    if report.diagnostics.iter().any(|d| d.publisher_name.as_deref() == Some(name)) {
        return Err(MfError::usage(
            format!("publisher '{name}' has configuration errors and cannot be used"),
            Some("run `mf publish target list` for details".to_string()),
        ));
    }

    // Fall back to mind.yaml targets
    let targets = config.publish.targets.as_deref().unwrap_or(&[]);
    let target = targets.iter().find(|t| t.name == name).ok_or_else(|| {
        MfError::not_found(
            format!("publish target '{name}' not found in mind.yaml or .mind-forge/publisher/"),
            Some("check the publisher name or `publish.targets[].name` in mind.yaml".to_string()),
        )
    })?;

    if !target.enabled {
        return Err(MfError::usage(
            format!("publish target '{}' is disabled", target.name),
            Some("set `enabled: true` on the target in mind.yaml".to_string()),
        ));
    }

    Ok(ResolvedPublisherTarget { target: target.clone() })
}
