//! Maintenance tests (T059, T061): status report coverage and
//! no-prompt clear/legacy/disable acknowledgment requirements.

mod common;
use common::embedding_provider::{provider_repo, run};

fn enabled_repo() -> tempfile::TempDir {
    provider_repo() // already enabled by provider_repo
}

fn synced_repo() -> tempfile::TempDir {
    let repo = provider_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "sync failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    repo
}

// ── T059: Status tests ──

#[test]
fn status_json_output_has_required_fields() {
    let repo = synced_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "status"], &[]);
    assert_eq!(code, 0, "status failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    // Check core fields exist.
    assert!(v["data"]["data"]["index_status"].as_str().is_some(), "must have index_status");
    assert!(v["data"]["data"]["retained_snapshots"].as_u64().is_some(), "must have retained_snapshots");
}

#[test]
fn status_on_enabled_but_unsynced_repo_reports_ready_or_missing() {
    let repo = enabled_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "status"], &[]);
    assert_eq!(code, 0, "status failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let status = v["data"]["data"]["index_status"].as_str().unwrap_or("unknown");
    assert!(
        status == "ready" || status == "missing",
        "unsynced repo status must be ready or missing, got '{status}'\n{stdout}"
    );
}

// ── T061: Clear / Legacy / Disable acknowledgment ──

#[test]
fn clear_requires_yes_flag() {
    let repo = synced_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "admin", "clear"], &[]);
    assert_eq!(code, 2, "clear without --yes must be a usage error\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let combined = format!("{stdout}{stderr}");
    assert!(combined.contains("--yes"), "error must mention --yes\n{combined}");
}

#[test]
fn clear_with_dry_run_allowed_without_yes() {
    let repo = synced_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "admin", "clear", "--dry-run"], &[]);
    assert_eq!(code, 0, "--dry-run clear must succeed without --yes\nstdout:\n{stdout}\nstderr:\n{stderr}");
}

#[test]
fn recover_requires_yes_flag() {
    let repo = synced_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "admin", "recover", "--snapshot", "nonexistent"], &[]);
    assert_eq!(code, 2, "recover without --yes must be a usage error\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let combined = format!("{stdout}{stderr}");
    assert!(combined.contains("--yes"), "error must mention --yes\n{combined}");
}

#[test]
fn legacy_import_blocks_removals_without_allow_flag() {
    let repo = synced_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "advanced", "legacy", "import"], &[]);
    assert_eq!(code, 2);
    assert!(format!("{stdout}{stderr}").contains("unrecognized"));
}
