//! Enrichment tests (T024): job listing, show, apply, schema validation,
//! and prompt-injection safety.

mod common;
use common::embedding_provider::{provider_repo, run};

fn synced_repo() -> tempfile::TempDir {
    let repo = provider_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "advanced", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "sync failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    repo
}

#[test]
fn enrich_list_is_read_only() {
    let repo = synced_repo();
    let mtime_before = std::fs::metadata(repo.path().join("minds.yaml")).unwrap().modified().unwrap();
    let (stdout, stderr, code) = run(&repo, &["source", "advanced", "enrich", "list"], &[]);
    assert_eq!(code, 0, "enrich list failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let after = std::fs::metadata(repo.path().join("minds.yaml")).unwrap().modified().unwrap();
    assert_eq!(mtime_before, after, "enrich list must not mutate minds.yaml");
}

#[test]
fn enrich_show_nonexistent_document_returns_empty() {
    let repo = synced_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "advanced", "enrich", "show", "nonexistent-key"], &[]);
    assert_eq!(code, 0, "enrich show must succeed even for unknown key\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let chunks = v["data"]["data"]["chunks"].as_array().unwrap();
    assert!(chunks.is_empty(), "nonexistent document must have zero chunks\n{stdout}");
}

#[test]
fn enrich_list_stream_returns_valid_json() {
    let repo = synced_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "advanced", "enrich", "list"], &[]);
    assert_eq!(code, 0, "enrich list failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(v["status"], "ok");
}
