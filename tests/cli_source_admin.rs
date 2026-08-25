//! CLI contract tests for storage schema compatibility (spec 071, spec 075).
//!
//! Spec 075 moved compatibility from a self-declared `minds.yaml` value to the
//! actual on-disk table structure (FR-002). These tests assert the consequence
//! directly: hand-editing the declared version has no effect at all — it is
//! not read, so it cannot lie (FR-010, SC-002). Coverage of an actually-stale
//! table (missing v2 columns) lives at the unit level in
//! `src/service/source/advanced/lance_store.rs`, where a genuine v1-shaped
//! table can be constructed directly; the CLI/subprocess boundary here has no
//! way to fake real table structure, which is the point.

mod common;
use common::embedding_provider::{provider_repo, run};

/// Rewrite `minds.yaml.source.storage_schema_version`. Spec 075: this value is
/// no longer part of `RepositorySourceConfig` at all, so writing it is inert —
/// it round-trips as an unknown key and is ignored by every code path.
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

#[test]
fn hand_edited_schema_version_does_not_affect_search() {
    let repo = provider_repo();
    let (o, e, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "activation sync failed\n{o}\n{e}");

    declare_stale_schema_version_in_yaml(repo.path());

    // FR-010/SC-002: the declared value is not read, so search is unaffected —
    // the previously-documented hand-edit workaround has nothing left to do.
    let (stdout, stderr, code) = run(&repo, &["source", "search", "notes"], &[]);
    assert_eq!(code, 0, "search must be unaffected by a hand-edited declaration\nstdout:\n{stdout}\nstderr:\n{stderr}");
}

#[test]
fn hand_edited_schema_version_does_not_affect_incremental_sync() {
    let repo = provider_repo();
    let (o, e, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "activation sync failed\n{o}\n{e}");

    declare_stale_schema_version_in_yaml(repo.path());

    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "sync must be unaffected by a hand-edited declaration\nstdout:\n{stdout}\nstderr:\n{stderr}");
}

#[test]
fn rebuild_succeeds_regardless_of_declared_schema_version() {
    let repo = provider_repo();
    let (o, e, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "activation sync failed\n{o}\n{e}");

    declare_stale_schema_version_in_yaml(repo.path());

    let (stdout, stderr, code) = run(&repo, &["source", "admin", "rebuild", "--offline"], &[]);
    assert_eq!(code, 0, "rebuild must succeed\nstdout:\n{stdout}\nstderr:\n{stderr}");

    let (stdout, stderr, code) = run(&repo, &["source", "search", "notes"], &[]);
    assert_eq!(code, 0, "search must succeed after rebuild\nstdout:\n{stdout}\nstderr:\n{stderr}");
}
