//! CLI contract tests for `source sync --rebuild` (spec 074 #33).
//!
//! On a schema-drifted Lance repo, plain `sync` refuses with a hint pointing at
//! `mf source sync --rebuild`; `sync --rebuild` regenerates the index to the
//! current storage schema in the same command family and restores read-only
//! advanced commands. `--help` lists the new flag.

mod common;
use common::embedding_provider::{provider_repo, run};

/// Rewrite `minds.yaml.source.storage_schema_version` to simulate a v1 snapshot
/// (schema drift from the current v2 build).
fn downgrade_schema_to_v1(repo: &std::path::Path) {
    let minds = repo.join("minds.yaml");
    let text = std::fs::read_to_string(&minds).expect("read minds.yaml");
    let mut root: serde_yaml::Value = serde_yaml::from_str(&text).expect("parse minds.yaml");
    let source = root
        .as_mapping_mut()
        .and_then(|m| m.get_mut(serde_yaml::Value::String("source".into())))
        .and_then(|s| s.as_mapping_mut())
        .expect("source block");
    source.insert("storage_schema_version".into(), serde_yaml::Value::String("1".into()));
    std::fs::write(&minds, serde_yaml::to_string(&root).unwrap()).expect("write minds.yaml");
}

/// T009: plain `source sync` on a schema-drifted repo refuses with a hint naming
/// `mf source sync --rebuild` (not the old `admin rebuild`).
#[test]
fn plain_sync_refuses_with_rebuild_hint() {
    let repo = provider_repo();
    let (o, e, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "activation sync failed\n{o}\n{e}");

    downgrade_schema_to_v1(repo.path());

    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 1, "drifted sync must refuse\nstdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(
        stderr.contains("mf source sync --rebuild"),
        "hint must name `mf source sync --rebuild`\nstderr:\n{stderr}"
    );
    assert!(!stderr.contains("admin rebuild"), "hint must not point at `admin rebuild`\nstderr:\n{stderr}");
}

/// T010: `source sync --rebuild` on the drifted repo regenerates the index,
/// warns about the full re-index, and a subsequent read-only advanced command
/// (search) succeeds.
#[test]
fn sync_rebuild_recovers_drifted_repo() {
    let repo = provider_repo();
    let (o, e, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "activation sync failed\n{o}\n{e}");

    downgrade_schema_to_v1(repo.path());

    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--rebuild", "--offline"], &[]);
    assert_eq!(code, 0, "sync --rebuild must succeed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let envelope: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON envelope");
    assert_eq!(envelope["status"], "ok", "status must be ok\n{stdout}");
    assert!(stderr.contains("full re-index"), "stderr must warn about the full re-index\nstderr:\n{stderr}");

    // The index is now queryable again without an admin detour.
    let (stdout, stderr, code) = run(&repo, &["source", "search", "notes"], &[]);
    assert_eq!(code, 0, "search must succeed after sync --rebuild\nstdout:\n{stdout}\nstderr:\n{stderr}");
}

/// T010b: `--rebuild --dry-run` reports without writing and without upgrading
/// the persisted schema marker.
#[test]
fn sync_rebuild_dry_run_does_not_write() {
    let repo = provider_repo();
    let (o, e, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "activation sync failed\n{o}\n{e}");

    downgrade_schema_to_v1(repo.path());

    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--rebuild", "--offline", "--dry-run"], &[]);
    assert_eq!(code, 0, "sync --rebuild --dry-run must succeed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(!stderr.contains("full re-index ran"), "dry-run must not claim a real re-index ran\nstderr:\n{stderr}");

    // The drift marker is still v1 (dry-run must not upgrade).
    let minds = repo.path().join("minds.yaml");
    let text = std::fs::read_to_string(&minds).unwrap();
    assert!(
        text.contains("storage_schema_version: \"1\"") || text.contains("storage_schema_version: '1'"),
        "dry-run must not upgrade the schema marker\n{text}"
    );
}

/// T011: `source sync --help` lists the `--rebuild` flag.
#[test]
fn sync_help_lists_rebuild_flag() {
    let repo = provider_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--help"], &[]);
    assert_eq!(code, 0, "sync --help failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(stdout.contains("--rebuild"), "help must list --rebuild\nstdout:\n{stdout}");
}
