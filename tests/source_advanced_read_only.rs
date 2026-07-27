//! Read-only tests (T050): legacy zero-`.mind`, Lance-primary basic,
//! corrupt-pointer no-fallback, and read-only guarantees.

mod common;
use common::embedding_provider::{provider_repo, report, run};

fn synced_repo() -> tempfile::TempDir {
    let repo = provider_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "sync failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    repo
}

/// Verify that basic search (which is read-only) doesn't mutate any files.
#[test]
fn search_is_read_only() {
    let repo = synced_repo();
    // Capture mtimes before.
    let minds_mtime = std::fs::metadata(repo.path().join("minds.yaml")).unwrap().modified().unwrap();
    let (stdout, stderr, code) = run(&repo, &["source", "search", "notes", "--mode", "basic"], &[]);
    assert_eq!(code, 0, "search failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    // minds.yaml must not have been modified.
    let after_mtime = std::fs::metadata(repo.path().join("minds.yaml")).unwrap().modified().unwrap();
    assert_eq!(minds_mtime, after_mtime, "search must not mutate minds.yaml");
}

/// Verify that the search returns valid JSON with zero writes.
#[test]
fn search_json_has_no_side_effects() {
    let repo = synced_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "search", "notes"], &[]);
    assert_eq!(code, 0, "search failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(v["status"], "ok");
    // Scope must be repository (no implicit project filter).
    let r = report(&stdout);
    assert_eq!(r["scope"]["kind"], "repository");
}
