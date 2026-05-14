use std::path::Path;

use crate::error::Result;

/// Publisher directory relative to the active Mind Repo root.
pub const PUBLISHER_DIR: &str = ".mind-forge/publisher";

pub fn scan(repo_root: &Path) -> Result<crate::service::publisher::PublisherDiscoveryReport> {
    let publisher_dir = repo_root.join(PUBLISHER_DIR);

    let mut publishers = Vec::new();
    let mut diagnostics = Vec::new();

    if !publisher_dir.is_dir() {
        return Ok(crate::service::publisher::PublisherDiscoveryReport { publishers, diagnostics });
    }

    let mut entries: Vec<_> = match std::fs::read_dir(&publisher_dir) {
        Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
        Err(_) => {
            return Ok(crate::service::publisher::PublisherDiscoveryReport { publishers, diagnostics });
        }
    };
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "yaml" && ext != "yml" {
            continue;
        }

        let _file_name = entry.file_name();
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                diagnostics.push(super::PublisherDiagnostic {
                    kind: super::PublisherDiagnosticKind::MalformedYaml,
                    path: Some(path.clone()),
                    publisher_name: None,
                    message: format!("cannot read publisher file: {e}"),
                    hint: Some(format!("check file permissions for {}", path.display())),
                });
                continue;
            }
        };

        let repo_rel = crate::service::util::repo_relative_path(repo_root, &path);
        super::validation::process_definition(&content, &stem, &repo_rel, &path, &mut publishers, &mut diagnostics);
    }

    publishers.sort_by(|a, b| a.name.cmp(&b.name));
    diagnostics.sort_by(|a, b| {
        a.path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .cmp(&b.path.as_ref().map(|p| p.to_string_lossy().to_string()))
            .then(format!("{:?}", a.kind).cmp(&format!("{:?}", b.kind)))
    });

    Ok(crate::service::publisher::PublisherDiscoveryReport { publishers, diagnostics })
}
