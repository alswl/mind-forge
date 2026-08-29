//! CLI contract tests for `source sync --rebuild` (spec 074 #33, spec 075).
//!
//! Spec 075 moved schema compatibility to the actual on-disk table structure
//! (FR-002): a hand-edited `minds.yaml.source.storage_schema_version` is no
//! longer read by anything, so it can no longer trigger — or fake escaping —
//! a drift refusal. Genuine drift (a real v1-shaped table) is covered at the
//! unit level in `src/service/source/advanced/lance_store.rs`, where a real
//! table can be constructed directly. `sync --rebuild` still succeeds and
//! restores search unconditionally; `--help` still lists the flag.

mod common;
use common::embedding_provider::{provider_repo, run};

/// Rewrite `minds.yaml.source.storage_schema_version`. Spec 075: this key is
/// no longer part of `RepositorySourceConfig`, so writing it is inert.
fn declare_stale_schema_version_in_yaml(repo: &std::path::Path) {
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

/// FR-010/SC-002: a hand-edited declaration cannot fake a drift refusal —
/// there is nothing left reading it.
#[test]
fn plain_sync_ignores_hand_edited_schema_declaration() {
    let repo = provider_repo();
    let (o, e, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "activation sync failed\n{o}\n{e}");

    declare_stale_schema_version_in_yaml(repo.path());

    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "sync must be unaffected by a hand-edited declaration\nstdout:\n{stdout}\nstderr:\n{stderr}");
}

/// `source sync --rebuild` succeeds and search remains available afterward,
/// regardless of what the declared (and now-inert) schema value claims.
#[test]
fn sync_rebuild_succeeds_and_search_stays_available() {
    let repo = provider_repo();
    let (o, e, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "activation sync failed\n{o}\n{e}");

    declare_stale_schema_version_in_yaml(repo.path());

    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--rebuild", "--offline"], &[]);
    assert_eq!(code, 0, "sync --rebuild must succeed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let envelope: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON envelope");
    assert_eq!(envelope["status"], "ok", "status must be ok\n{stdout}");
    assert!(stderr.contains("full re-index"), "stderr must warn about the full re-index\nstderr:\n{stderr}");

    let (stdout, stderr, code) = run(&repo, &["source", "search", "notes"], &[]);
    assert_eq!(code, 0, "search must succeed after sync --rebuild\nstdout:\n{stdout}\nstderr:\n{stderr}");
}

/// FR-013: `--rebuild --dry-run` reports without writing content, the store,
/// or `minds.yaml` at all.
#[test]
fn sync_rebuild_dry_run_does_not_write() {
    let repo = provider_repo();
    let (o, e, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "activation sync failed\n{o}\n{e}");

    let minds = repo.path().join("minds.yaml");
    let before = std::fs::read(&minds).unwrap();

    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--rebuild", "--offline", "--dry-run"], &[]);
    assert_eq!(code, 0, "sync --rebuild --dry-run must succeed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(!stderr.contains("full re-index ran"), "dry-run must not claim a real re-index ran\nstderr:\n{stderr}");

    let after = std::fs::read(&minds).unwrap();
    assert_eq!(before, after, "dry-run must not write minds.yaml at all");
}

/// T011: `source sync --help` lists the `--rebuild` flag.
#[test]
fn sync_help_lists_rebuild_flag() {
    let repo = provider_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--help"], &[]);
    assert_eq!(code, 0, "sync --help failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(stdout.contains("--rebuild"), "help must list --rebuild\nstdout:\n{stdout}");
}
