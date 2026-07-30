//! CLI contract tests for storage schema migration (spec 071).
//!
//! Activating writes the current schema version. A snapshot pinned at an older
//! version must produce an actionable rebuild diagnostic (exit 1) rather than
//! being read silently under the old schema; `rebuild` then restores service.

mod common;
use common::embedding_provider::{provider_repo, run};

/// Rewrite `minds.yaml.source.storage_schema_version` to simulate a v1 snapshot.
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

#[test]
fn v1_snapshot_rejects_search_with_rebuild_diagnostic() {
    let repo = provider_repo();
    let (o, e, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "activation sync failed\n{o}\n{e}");

    downgrade_schema_to_v1(repo.path());

    let (stdout, stderr, code) = run(&repo, &["source", "search", "notes"], &[]);
    assert_eq!(code, 1, "v1 snapshot search must exit 1\nstdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(stderr.contains("rebuild"), "diagnostic must direct the operator to rebuild\nstderr:\n{stderr}");
    assert!(stderr.contains("schema"), "diagnostic must name the schema mismatch\nstderr:\n{stderr}");
}

#[test]
fn v1_snapshot_rejects_incremental_sync() {
    let repo = provider_repo();
    let (o, e, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "activation sync failed\n{o}\n{e}");

    downgrade_schema_to_v1(repo.path());

    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 1, "v1 snapshot incremental sync must exit 1\nstdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(stderr.contains("rebuild"), "sync diagnostic must direct to rebuild\nstderr:\n{stderr}");
}

#[test]
fn rebuild_upgrades_v1_snapshot_and_restores_service() {
    let repo = provider_repo();
    let (o, e, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "activation sync failed\n{o}\n{e}");

    downgrade_schema_to_v1(repo.path());

    // Rebuild is the migration path: it must run on a v1 snapshot.
    let (stdout, stderr, code) = run(&repo, &["source", "admin", "rebuild", "--offline"], &[]);
    assert_eq!(code, 0, "rebuild must succeed on a v1 snapshot\nstdout:\n{stdout}\nstderr:\n{stderr}");

    // After rebuild the marker is v2 and search serves again.
    let (stdout, stderr, code) = run(&repo, &["source", "search", "notes"], &[]);
    assert_eq!(code, 0, "search must succeed after rebuild\nstdout:\n{stdout}\nstderr:\n{stderr}");
}
