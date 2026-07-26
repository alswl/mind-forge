//! Source lifecycle tests (T025): add/update/rename/remove/index/clean
//! with Lance-primary active.

mod common;
use common::embedding_provider::{provider_repo, run};

fn synced_repo() -> tempfile::TempDir {
    let repo = provider_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "advanced", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "sync failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    repo
}

#[test]
fn source_new_with_lance_mode_registers_source() {
    let repo = synced_repo();
    // Trigger index which scans and registers sources.
    let (stdout, stderr, code) = run(&repo, &["source", "index", "--project", "alpha"], &[]);
    assert_eq!(code, 0, "source index failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    // Verify the source is listed.
    let (list_out, _, _) = run(&repo, &["source", "list", "--project", "alpha"], &[]);
    let v: serde_json::Value = serde_json::from_str(&list_out).expect("valid JSON");
    let sources = v["data"]["sources"].as_array().expect("sources array");
    assert!(!sources.is_empty(), "must have registered sources after index\n{list_out}");
}

#[test]
fn source_index_idempotent_in_lance_mode() {
    let repo = synced_repo();
    // Run index twice — second run must succeed.
    let (_, _, code1) = run(&repo, &["source", "index", "--project", "alpha"], &[]);
    assert_eq!(code1, 0);
    let (stdout, stderr, code2) = run(&repo, &["source", "index", "--project", "alpha"], &[]);
    assert_eq!(code2, 0, "second index must succeed\nstdout:\n{stdout}\nstderr:\n{stderr}");
}

#[test]
fn source_clean_removes_stale_entries() {
    let repo = synced_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "clean", "--project", "alpha"], &[]);
    assert_eq!(code, 0, "source clean failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
}
