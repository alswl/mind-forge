use std::path::PathBuf;

pub enum Verb {
    Create,
    Add,
    Rename,
    Remove,
    Update,
    Move,
    Index,
    Lint,
}

pub struct VerbResult {
    pub verb: Verb,
    pub kind: &'static str,
    pub identity: String,
    pub old_identity: Option<String>,
    pub path: Option<String>,
    pub dry_run: bool,
    pub details: serde_json::Value,
}

#[derive(Default)]
pub struct VerbOpts {
    pub emit_hyperlinks: bool,
    pub repo_root: Option<PathBuf>,
}

impl VerbOpts {
    pub fn from_repo_root(repo_root: Option<&std::path::Path>) -> Self {
        let profile = super::capability::build_profile();
        let policy = crate::model::terminal::OutputRenderingPolicy::from_profile(
            &profile,
            crate::model::terminal::OutputFormat::Text,
        );
        Self { emit_hyperlinks: policy.emit_hyperlinks, repo_root: repo_root.map(|p| p.to_path_buf()) }
    }
}

pub fn render_text(result: &VerbResult, opts: &VerbOpts) -> String {
    if result.dry_run {
        return match &result.verb {
            Verb::Create => format!("[dry-run] would create {}: {}", result.kind, result.identity),
            Verb::Add => format!("[dry-run] would add {}: {}", result.kind, result.identity),
            Verb::Rename => {
                let old = result.old_identity.as_deref().unwrap_or("?");
                format!("[dry-run] would rename {}: {} → {}", result.kind, old, result.identity)
            }
            Verb::Remove => format!("[dry-run] would remove {}: {}", result.kind, result.identity),
            Verb::Update => {
                let n = count_changes(&result.details);
                format!(
                    "[dry-run] would update {}: {} ({} field{})",
                    result.kind,
                    result.identity,
                    n,
                    if n == 1 { "" } else { "s" }
                )
            }
            Verb::Move => {
                let from = result.details.get("from_scope").and_then(|v| v.as_str()).unwrap_or("?");
                let to = result.details.get("to_scope").and_then(|v| v.as_str()).unwrap_or("?");
                format!("[dry-run] would move {}: {} ({} → {})", result.kind, result.identity, from, to)
            }
            Verb::Index => render_index_text(result),
            Verb::Lint => render_lint_text(result, opts),
        };
    }

    match &result.verb {
        Verb::Create => format!("✓ created {}: {}", result.kind, result.identity),
        Verb::Add => format!("✓ added {}: {}", result.kind, result.identity),
        Verb::Rename => {
            let old = result.old_identity.as_deref().unwrap_or("?");
            format!("✓ renamed {}: {} → {}", result.kind, old, result.identity)
        }
        Verb::Remove => format!("✓ removed {}: {}", result.kind, result.identity),
        Verb::Update => {
            let n = count_changes(&result.details);
            format!("✓ updated {}: {} ({} field{})", result.kind, result.identity, n, if n == 1 { "" } else { "s" })
        }
        Verb::Move => {
            let from = result.details.get("from_scope").and_then(|v| v.as_str()).unwrap_or("?");
            let to = result.details.get("to_scope").and_then(|v| v.as_str()).unwrap_or("?");
            format!("✓ moved {}: {} ({} → {})", result.kind, result.identity, from, to)
        }
        Verb::Index => render_index_text(result),
        Verb::Lint => render_lint_text(result, opts),
    }
}

fn count_changes(details: &serde_json::Value) -> usize {
    match details.get("changes") {
        Some(serde_json::Value::Object(map)) => map.len(),
        _ => 0,
    }
}

fn render_index_text(result: &VerbResult) -> String {
    let prefix = if result.dry_run { "[dry-run] " } else { "" };
    let (added, removed, kept) = extract_index_counts(&result.details);
    format!("{prefix}indexed {}: +{added} ={kept} -{removed}", result.kind)
}

fn render_lint_text(result: &VerbResult, opts: &VerbOpts) -> String {
    let mut out = String::new();
    if let Some(issues) = result.details.get("issues").and_then(|v| v.as_array()) {
        for issue in issues {
            let severity = issue.get("severity").and_then(|v| v.as_str()).unwrap_or("unknown");
            let kind = issue.get("kind").and_then(|v| v.as_str()).unwrap_or("unknown");
            let message = issue.get("message").and_then(|v| v.as_str()).unwrap_or("");
            let path = issue.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let line = issue.get("line").and_then(|v| v.as_u64()).unwrap_or(0);
            let linked_path = super::link::render_path_link(path, opts.repo_root.as_deref(), opts.emit_hyperlinks);
            if line > 0 {
                out.push_str(&format!("[{severity}] {kind}: {message} ({linked_path}:{line})\n"));
            } else {
                out.push_str(&format!("[{severity}] {kind}: {message} ({linked_path})\n"));
            }
        }
    }
    if let Some(summary) = result.details.get("summary") {
        let errors = summary.get("errors").and_then(|v| v.as_u64()).unwrap_or(0);
        let warnings = summary.get("warnings").and_then(|v| v.as_u64()).unwrap_or(0);
        let info = summary.get("info").and_then(|v| v.as_u64()).unwrap_or(0);
        let fixed = summary.get("fixed").and_then(|v| v.as_u64()).unwrap_or(0);
        if fixed > 0 {
            out.push_str(&format!("{errors} errors, {warnings} warnings, {info} info, {fixed} fixed\n"));
        } else {
            out.push_str(&format!("{errors} errors, {warnings} warnings, {info} info\n"));
        }
    }
    out
}

fn extract_index_counts(details: &serde_json::Value) -> (usize, usize, usize) {
    let added = details.get("added").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
    let removed = details.get("removed").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
    let kept = details.get("kept_count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    (added, removed, kept)
}

pub fn json_envelope(result: &VerbResult) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    map.insert("kind".to_string(), serde_json::Value::String(result.kind.to_string()));
    map.insert("identity".to_string(), serde_json::Value::String(result.identity.clone()));
    map.insert("dry_run".to_string(), serde_json::Value::Bool(result.dry_run));

    match &result.verb {
        Verb::Rename => {
            map.insert(
                "old_identity".to_string(),
                serde_json::Value::String(result.old_identity.clone().unwrap_or_default()),
            );
            map.insert("new_identity".to_string(), serde_json::Value::String(result.identity.clone()));
        }
        Verb::Remove => {
            map.insert("removed".to_string(), serde_json::Value::Bool(true));
        }
        _ => {}
    }

    if let Some(ref path) = result.path {
        map.insert("path".to_string(), serde_json::Value::String(path.clone()));
    }

    if !result.details.is_null() && !matches!(&result.verb, Verb::Index | Verb::Lint) {
        map.insert("details".to_string(), result.details.clone());
    }

    if matches!(&result.verb, Verb::Index | Verb::Lint)
        && !result.details.is_null()
        && let Some(obj) = result.details.as_object()
    {
        for (k, v) in obj {
            if !map.contains_key(k) {
                map.insert(k.clone(), v.clone());
            }
        }
    }

    serde_json::Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_text() {
        let r = VerbResult {
            verb: Verb::Create,
            kind: "project",
            identity: "demo".into(),
            old_identity: None,
            path: Some("projects/demo".into()),
            dry_run: false,
            details: serde_json::json!({"scaffolded": ["docs"]}),
        };
        assert_eq!(render_text(&r, &VerbOpts::default()), "✓ created project: demo");
    }

    #[test]
    fn create_dry_run_text() {
        let r = VerbResult {
            verb: Verb::Create,
            kind: "project",
            identity: "demo".into(),
            old_identity: None,
            path: Some("projects/demo".into()),
            dry_run: true,
            details: serde_json::json!({}),
        };
        assert_eq!(render_text(&r, &VerbOpts::default()), "[dry-run] would create project: demo");
    }

    #[test]
    fn rename_text() {
        let r = VerbResult {
            verb: Verb::Rename,
            kind: "project",
            identity: "demo-renamed".into(),
            old_identity: Some("demo".into()),
            path: None,
            dry_run: false,
            details: serde_json::json!({}),
        };
        assert_eq!(render_text(&r, &VerbOpts::default()), "✓ renamed project: demo → demo-renamed");
    }

    #[test]
    fn remove_text() {
        let r = VerbResult {
            verb: Verb::Remove,
            kind: "article",
            identity: "docs/draft".into(),
            old_identity: None,
            path: None,
            dry_run: false,
            details: serde_json::json!({}),
        };
        assert_eq!(render_text(&r, &VerbOpts::default()), "✓ removed article: docs/draft");
    }

    #[test]
    fn update_text() {
        let r = VerbResult {
            verb: Verb::Update,
            kind: "source",
            identity: "report".into(),
            old_identity: None,
            path: None,
            dry_run: false,
            details: serde_json::json!({"changes": {"title": {"from": "Old", "to": "New"}}}),
        };
        assert_eq!(render_text(&r, &VerbOpts::default()), "✓ updated source: report (1 field)");
    }

    #[test]
    fn index_text() {
        let r = VerbResult {
            verb: Verb::Index,
            kind: "article",
            identity: String::new(),
            old_identity: None,
            path: None,
            dry_run: false,
            details: serde_json::json!({"added": [{"identity": "a"}], "removed": [{"identity": "b"}], "kept_count": 5, "scanned_count": 7}),
        };
        assert_eq!(render_text(&r, &VerbOpts::default()), "indexed article: +1 =5 -1");
    }

    #[test]
    fn lint_text() {
        let r = VerbResult {
            verb: Verb::Lint,
            kind: "article",
            identity: String::new(),
            old_identity: None,
            path: None,
            dry_run: false,
            details: serde_json::json!({
                "issues": [
                    {"severity": "error", "kind": "missing_directory", "message": "docs/foo not on disk", "path": "docs/foo", "line": 0}
                ],
                "summary": {"errors": 1, "warnings": 0, "info": 0, "fixed": 0}
            }),
        };
        let out = render_text(&r, &VerbOpts::default());
        assert!(out.contains("[error] missing_directory: docs/foo not on disk (docs/foo)"));
        assert!(out.contains("1 errors, 0 warnings, 0 info"));
    }

    #[test]
    fn lint_path_renders_as_link() {
        let r = VerbResult {
            verb: Verb::Lint,
            kind: "article",
            identity: String::new(),
            old_identity: None,
            path: None,
            dry_run: false,
            details: serde_json::json!({
                "issues": [
                    {"severity": "error", "kind": "missing_directory", "message": "docs/foo not on disk", "path": "docs/foo", "line": 42}
                ],
                "summary": {"errors": 1, "warnings": 0, "info": 0, "fixed": 0}
            }),
        };
        let opts = VerbOpts { emit_hyperlinks: true, repo_root: Some(PathBuf::from("/repo")) };
        let out = render_text(&r, &opts);
        assert!(out.contains("\x1b]8;;file:///repo/docs/foo\x1b\\"));
        // OSC 8 wraps the path; the ":42" follows the closing escape
        assert!(out.contains("\x1b]8;;\x1b\\:42"));
    }

    #[test]
    fn lint_path_no_hyperlinks_returns_plain() {
        let r = VerbResult {
            verb: Verb::Lint,
            kind: "article",
            identity: String::new(),
            old_identity: None,
            path: None,
            dry_run: false,
            details: serde_json::json!({
                "issues": [
                    {"severity": "error", "kind": "missing_directory", "message": "docs/foo not on disk", "path": "docs/foo", "line": 0}
                ],
                "summary": {"errors": 1, "warnings": 0, "info": 0, "fixed": 0}
            }),
        };
        let out = render_text(&r, &VerbOpts::default());
        assert!(!out.contains('\x1b'));
        assert!(out.contains("docs/foo"));
    }

    #[test]
    fn json_envelope_create() {
        let r = VerbResult {
            verb: Verb::Create,
            kind: "project",
            identity: "demo".into(),
            old_identity: None,
            path: Some("projects/demo".into()),
            dry_run: false,
            details: serde_json::json!({"scaffolded": ["docs"]}),
        };
        let v = json_envelope(&r);
        assert_eq!(v["kind"], "project");
        assert_eq!(v["identity"], "demo");
        assert_eq!(v["dry_run"], false);
        assert_eq!(v["path"], "projects/demo");
        assert_eq!(v["details"]["scaffolded"][0], "docs");
    }

    #[test]
    fn json_envelope_rename() {
        let r = VerbResult {
            verb: Verb::Rename,
            kind: "term",
            identity: "RAG".into(),
            old_identity: Some("rag".into()),
            path: None,
            dry_run: false,
            details: serde_json::json!({"keep_alias": true}),
        };
        let v = json_envelope(&r);
        assert_eq!(v["kind"], "term");
        assert_eq!(v["new_identity"], "RAG");
        assert_eq!(v["old_identity"], "rag");
        assert_eq!(v["details"]["keep_alias"], true);
    }

    #[test]
    fn json_envelope_remove() {
        let r = VerbResult {
            verb: Verb::Remove,
            kind: "article",
            identity: "docs/draft".into(),
            old_identity: None,
            path: None,
            dry_run: false,
            details: serde_json::json!({}),
        };
        let v = json_envelope(&r);
        assert_eq!(v["removed"], true);
    }
}
