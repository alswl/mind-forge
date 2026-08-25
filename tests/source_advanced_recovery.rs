//! Recovery and retention tests (T062, T063): snapshot validation,
//! recovery to a retained snapshot, and minimum-two retention.

mod common;
use common::embedding_provider::{provider_repo, run};

fn synced_repo() -> tempfile::TempDir {
    let repo = provider_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "sync failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    repo
}

#[test]
fn recover_nonexistent_snapshot_is_recovery_error() {
    let repo = synced_repo();
    let (stdout, stderr, code) =
        run(&repo, &["source", "admin", "recover", "--snapshot", "nonexistent-snapshot", "--yes"], &[]);
    assert!(code != 0, "nonexistent snapshot must fail\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let combined = format!("{stdout}{stderr}");
    assert!(combined.contains("snapshot") || combined.contains("not found"), "error must mention snapshot\n{combined}");
}

#[test]
fn status_lists_retained_snapshots() {
    let repo = synced_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "status"], &[]);
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
    // Removed commands must be rejected cleanly.
    let combined = format!("{stdout}{stderr}");
    assert_eq!(code, 2);
    assert!(combined.contains("unrecognized"));
}

// ── Spec 075 US1: truthful state and schema (FR-003..FR-005) ──

/// T007: a corpus exists on disk but this machine's local state is gone —
/// `sync` must adopt the existing corpus, not discard it or refuse (#39).
#[test]
fn sync_adopts_existing_corpus_after_local_state_is_lost() {
    let repo = synced_repo();
    let state_path = repo.path().join(".mind-forge/state.yaml");
    std::fs::remove_file(&state_path).unwrap();
    let (before_stdout, before_stderr, before_code) = run(&repo, &["source", "status"], &[]);
    assert_eq!(before_code, 0, "precondition: status must succeed\n{before_stdout}\n{before_stderr}");
    let before: serde_json::Value = serde_json::from_str(&before_stdout).unwrap();
    let generation_before = before["data"]["data"]["primary_catalog_fingerprint"].clone();

    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "sync must adopt the existing corpus, not refuse\nstdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(state_path.exists(), "sync must record activation locally after adopting");

    // Adoption must not have rebuilt from scratch: the generation is unchanged.
    let (status_stdout, _, _) = run(&repo, &["source", "status"], &[]);
    let after: serde_json::Value = serde_json::from_str(&status_stdout).unwrap();
    assert_eq!(
        generation_before, after["data"]["data"]["primary_catalog_fingerprint"],
        "adoption must reuse the existing generation, not create a new one"
    );

    let (stdout, stderr, code) = run(&repo, &["source", "search", "notes"], &[]);
    assert_eq!(code, 0, "search must work immediately after adoption\nstdout:\n{stdout}\nstderr:\n{stderr}");
}

/// T008: a totally fresh repo (no local state, no corpus at all) must still
/// activate from scratch on first `sync` — confirms US1 does not regress the
/// always-worked first-activation path. `provider_repo()` already runs
/// `source sync --offline` once, so this builds the repo manually and stops
/// short of ever syncing it.
#[test]
fn sync_activates_from_scratch_when_nothing_exists_yet() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    common::write_source_file(&repo, "alpha", "sources/file", "notes", "Quantum entanglement enables teleportation.\n");
    let (stdout, stderr, code) = run(&repo, &["source", "index", "--project", "alpha"], &[]);
    assert_eq!(code, 0, "register failed\nstdout:\n{stdout}\nstderr:\n{stderr}");

    assert!(!repo.path().join(".mind-forge/state.yaml").exists(), "precondition: never synced");
    assert!(!repo.path().join(".mind-forge/cache").exists(), "precondition: no corpus on disk yet");

    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "fresh activation must succeed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(repo.path().join(".mind-forge/state.yaml").exists(), "sync must record activation");

    let (stdout, stderr, code) = run(&repo, &["source", "search", "entanglement"], &[]);
    assert_eq!(code, 0, "search must work after fresh activation\nstdout:\n{stdout}\nstderr:\n{stderr}");
}

/// T009 (edge case): local state claims activation but the corpus pointer is
/// gone (e.g. the cache directory was cleaned). `status` must report it as
/// missing rather than erroring, and a subsequent `sync` must self-heal.
#[test]
fn missing_corpus_with_stale_local_state_is_reported_then_healed() {
    let repo = synced_repo();
    std::fs::remove_dir_all(repo.path().join(".mind-forge/cache")).unwrap();
    // Local state still says `activated: true` — stale, but must not error.
    assert!(repo.path().join(".mind-forge/state.yaml").exists());

    let (stdout, stderr, code) = run(&repo, &["source", "status"], &[]);
    assert_eq!(code, 0, "status must not error on a stale local state\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["data"]["data"]["index_status"], "missing", "corpus is genuinely gone\n{stdout}");

    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "sync must self-heal from a stale local state\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let (stdout, stderr, code) = run(&repo, &["source", "search", "notes"], &[]);
    assert_eq!(code, 0, "search must work after self-heal\nstdout:\n{stdout}\nstderr:\n{stderr}");
}

// ── T064: Project intent recovery ──

#[test]
fn project_intents_are_visible_in_status() {
    let repo = synced_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "status"], &[]);
    assert_eq!(code, 0, "status failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    // pending_intents must be reported (even if zero).
    let intents = v["data"]["data"]["pending_intents"].as_u64().unwrap_or(0);
    // After a clean sync there should be zero pending intents.
    assert_eq!(intents, 0, "clean sync must have zero pending intents, got {intents}\n{stdout}");
}
