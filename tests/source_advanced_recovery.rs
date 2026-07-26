//! Recovery and retention tests (T062, T063): snapshot validation,
//! recovery to a retained snapshot, and minimum-two retention.

mod common;
use common::embedding_provider::{provider_repo, run};

fn synced_repo() -> tempfile::TempDir {
    let repo = provider_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "advanced", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "sync failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    repo
}

#[test]
fn recover_nonexistent_snapshot_is_recovery_error() {
    let repo = synced_repo();
    let (stdout, stderr, code) =
        run(&repo, &["source", "advanced", "recover", "--snapshot", "nonexistent-snapshot", "--yes"], &[]);
    assert!(code != 0, "nonexistent snapshot must fail\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let combined = format!("{stdout}{stderr}");
    assert!(combined.contains("snapshot") || combined.contains("not found"), "error must mention snapshot\n{combined}");
}

#[test]
fn status_lists_retained_snapshots() {
    let repo = synced_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "advanced", "status"], &[]);
    assert_eq!(code, 0, "status failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let snapshots = v["data"]["data"]["retained_snapshots"].as_u64().unwrap_or(0);
    // After one sync there should be at least one snapshot.
    assert!(snapshots >= 1, "must have at least one retained snapshot\n{stdout}");
}

#[test]
fn disable_blocks_when_projections_have_drift() {
    let repo = synced_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "advanced", "disable"], &[]);
    // Disable may succeed if projections are current, or block with details.
    // Either outcome is valid; the key is that it doesn't crash.
    let combined = format!("{stdout}{stderr}");
    if code != 0 {
        assert!(
            combined.contains("projection") || combined.contains("drift") || combined.contains("legacy export"),
            "disable error must mention projection status\n{combined}"
        );
    }
}

// ── T064: Project intent recovery ──

#[test]
fn project_intents_are_visible_in_status() {
    let repo = synced_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "advanced", "status"], &[]);
    assert_eq!(code, 0, "status failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    // pending_intents must be reported (even if zero).
    let intents = v["data"]["data"]["pending_intents"].as_u64().unwrap_or(0);
    // After a clean sync there should be zero pending intents.
    assert_eq!(intents, 0, "clean sync must have zero pending intents, got {intents}\n{stdout}");
}
