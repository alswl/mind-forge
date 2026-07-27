//! CLI contract tests for `mf source search` (T047): text/JSON output,
//! exit codes, mode resolution, and deterministic degradation.

mod common;
use common::embedding_provider::{provider_repo, report, run};

fn synced_repo() -> tempfile::TempDir {
    let repo = provider_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "sync failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    repo
}

#[test]
fn search_json_output_has_envelope() {
    let repo = synced_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "search", "entanglement"], &[]);
    assert_eq!(code, 0, "search failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON envelope");
    assert_eq!(v["status"], "ok");
    // Search uses the report() helper which expects envelope.data.data.
    let r = report(&stdout);
    assert!(r["results"].as_array().is_some(), "results must be an array\n{stdout}");
}

#[test]
fn search_basic_mode_returns_metadata_results() {
    let repo = synced_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "search", "notes", "--mode", "basic"], &[]);
    assert_eq!(code, 0, "basic search failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let r = report(&stdout);
    let results = r["results"].as_array().expect("results array");
    assert!(!results.is_empty(), "basic search must return metadata hits\n{stdout}");
    for item in results {
        let paths: Vec<&str> =
            item["retrieval_paths"].as_array().map_or(vec![], |a| a.iter().filter_map(|p| p.as_str()).collect());
        assert!(paths.iter().any(|p| p.contains("basic")), "basic result must have basic retrieval path\n{item:#?}");
    }
}

#[test]
fn search_advanced_mode_returns_content_results() {
    let repo = synced_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "search", "entanglement", "--mode", "advanced"], &[]);
    assert_eq!(code, 0, "advanced search failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let r = report(&stdout);
    let results = r["results"].as_array().expect("results array");
    assert!(!results.is_empty(), "advanced search must return hits (possibly degraded)\n{stdout}");
    if !r["degraded"].as_bool().unwrap_or(false) {
        for item in results {
            let paths: Vec<&str> =
                item["retrieval_paths"].as_array().map_or(vec![], |a| a.iter().filter_map(|p| p.as_str()).collect());
            assert!(
                paths.iter().any(|p| p.contains("advanced")),
                "non-degraded advanced result must have advanced retrieval path\n{item:#?}"
            );
        }
    }
}

#[test]
fn search_both_mode_returns_fused_results() {
    let repo = synced_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "search", "entanglement", "--mode", "both"], &[]);
    assert_eq!(code, 0, "both search failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let r = report(&stdout);
    assert!(!r["results"].as_array().unwrap().is_empty(), "both search must return hits\n{stdout}");
}

#[test]
fn search_empty_query_succeeds() {
    let repo = synced_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "search", ""], &[]);
    assert_eq!(code, 0, "empty query search failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON envelope");
    assert_eq!(v["status"], "ok", "empty query must succeed\n{stdout}");
}

#[test]
fn search_respects_limit() {
    let repo = synced_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "search", "entanglement", "--limit", "2"], &[]);
    assert_eq!(code, 0, "search failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let r = report(&stdout);
    let results = r["results"].as_array().expect("results array");
    assert!(results.len() <= 2, "limit must be respected, got {}\n{stdout}", results.len());
}

#[test]
fn search_with_integer_revision() {
    let repo = synced_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "search", "entanglement", "--revision", "1"], &[]);
    assert_eq!(code, 0, "revision search failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON envelope");
    assert_eq!(v["status"], "ok");
}

#[test]
fn search_with_date_revision_resolves_correctly() {
    let repo = synced_repo();
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let (stdout, stderr, code) = run(&repo, &["source", "search", "entanglement", "--revision", &today], &[]);
    assert_eq!(code, 0, "date revision search failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON envelope");
    assert_eq!(v["status"], "ok");
}

#[test]
fn search_invalid_revision_is_usage_error() {
    let repo = synced_repo();
    let (stdout, stderr, code) =
        run(&repo, &["source", "search", "entanglement", "--revision", "not-a-revision-or-date"], &[]);
    assert_eq!(code, 2, "invalid revision must be a usage error (exit 2)\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("invalid revision") || combined.contains("revision"),
        "error must mention revision\n{combined}"
    );
}

#[test]
fn search_text_output_is_default_when_no_format_flag() {
    let repo = synced_repo();
    let mut cmd = assert_cmd::Command::cargo_bin("mf").expect("mf binary");
    cmd.arg("--root").arg(repo.path()).args(["source", "search", "entanglement"]);
    let output = cmd.output().expect("run mf");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert_eq!(output.status.code().unwrap_or(-1), 0, "search failed\nstdout:\n{stdout}");
    assert!(!stdout.trim().starts_with('{'), "default output must be text, not JSON\n{stdout}");
    assert!(
        stdout.contains("SCORE") || stdout.contains("results"),
        "text output must contain table or summary\n{stdout}"
    );
}
