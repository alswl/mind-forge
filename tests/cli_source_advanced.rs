//! CLI configuration-contract tests for the advanced Source embedding
//! provider (T089): configuration validation, deterministic search
//! degradation without a usable provider, and offline enforcement.

mod common;
use common::embedding_provider::{
    Behavior, KEY_ENV, MockProvider, SECRET, configure_embedding, configure_provider, provider_repo, report, run,
};

#[test]
fn sync_requires_embedding_model_when_endpoint_is_configured() {
    let repo = provider_repo();
    configure_embedding(repo.path(), &[("embedding_endpoint", "http://127.0.0.1:9".into())]);

    let (stdout, stderr, code) = run(&repo, &["source", "sync"], &[]);
    assert_eq!(code, 2, "endpoint without model must be a usage error\nstdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(
        format!("{stdout}{stderr}").contains("embedding_model"),
        "error must name the missing key\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[test]
fn advanced_search_degrades_deterministically_without_a_provider() {
    let repo = provider_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "offline sync failed\nstdout:\n{stdout}\nstderr:\n{stderr}");

    let (stdout, stderr, code) = run(&repo, &["source", "search", "entanglement", "--mode", "advanced"], &[]);
    assert_eq!(code, 0, "unconfigured provider must degrade, not fail\nstdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(
        format!("{stdout}{stderr}").contains("not configured"),
        "degradation must be announced\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    let report = report(&stdout);
    assert!(
        !report["results"].as_array().expect("results array").is_empty(),
        "keyword retrieval must still return hits\n{stdout}"
    );
}

#[test]
fn advanced_search_with_invalid_provider_config_degrades_with_warning() {
    let repo = provider_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "offline sync failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    configure_embedding(repo.path(), &[("embedding_endpoint", "http://127.0.0.1:9".into())]);

    let (stdout, stderr, code) = run(&repo, &["source", "search", "entanglement", "--mode", "advanced"], &[]);
    assert_eq!(
        code, 0,
        "invalid provider config must degrade search, not fail it\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        format!("{stdout}{stderr}").contains("semantic retrieval degraded"),
        "degradation must be announced\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[test]
fn advanced_search_with_missing_credential_degrades_not_fails() {
    let repo = provider_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "offline sync failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    configure_provider(repo.path(), "http://127.0.0.1:9");

    let (stdout, stderr, code) = run(&repo, &["source", "search", "entanglement", "--mode", "advanced"], &[]);
    assert_eq!(code, 0, "missing credential must degrade search, not fail it\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let combined = format!("{stdout}{stderr}");
    assert!(combined.contains("semantic retrieval degraded"), "degradation must be announced\n{combined}");
    assert!(combined.contains(KEY_ENV), "warning must name the missing env var\n{combined}");
}

#[test]
fn offline_sync_makes_no_provider_requests() {
    let mock = MockProvider::start(Behavior::Vectors(384));
    let repo = provider_repo();
    configure_provider(repo.path(), &mock.endpoint);

    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--offline"], &[(KEY_ENV, SECRET)]);
    assert_eq!(code, 0, "offline sync failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    assert_eq!(
        mock.request_count(),
        0,
        "--offline must not contact the embedding provider\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

// ── Spec 075 US1 T015/T016: machine-local state is minimal, registration is unconditional ──

/// T015 (FR-001): machine-local state carries only activation status — no
/// snapshot id, catalog fingerprint, or schema version.
#[test]
fn local_state_carries_only_activation_status() {
    let repo = provider_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "sync failed\nstdout:\n{stdout}\nstderr:\n{stderr}");

    let state = std::fs::read_to_string(repo.path().join(".mind-forge/state.yaml")).unwrap();
    assert!(state.contains("activated"), "local state must record activation:\n{state}");
    assert!(!state.contains("activation_snapshot_id"), "no snapshot id may be recorded locally:\n{state}");
    assert!(
        !state.contains("activation_catalog_fingerprint"),
        "no catalog fingerprint may be recorded locally:\n{state}"
    );
    assert!(!state.contains("storage_schema_version"), "no schema version may be recorded locally:\n{state}");
}

/// T016 (#37 regression guard): registration commands never depend on
/// activation state — they must not emit the old "activation marker is
/// incomplete" refusal, with or without local state present.
#[test]
fn registration_commands_never_emit_the_incomplete_marker_error() {
    let repo = provider_repo();
    std::fs::remove_file(repo.path().join(".mind-forge/state.yaml")).unwrap();

    let f = repo.path().join("projects/alpha/sources/file/extra.md");
    std::fs::write(&f, "extra content\n").unwrap();
    let (stdout, stderr, code) =
        run(&repo, &["source", "new", f.to_str().unwrap(), "--project", "alpha", "--register-only"], &[]);
    assert!(!format!("{stdout}{stderr}").contains("activation marker is incomplete"), "{stdout}\n{stderr}");
    assert_eq!(code, 0, "register-only must succeed without local state\nstdout:\n{stdout}\nstderr:\n{stderr}");

    let (stdout, stderr, code) = run(&repo, &["source", "index", "--project", "alpha"], &[]);
    assert!(!format!("{stdout}{stderr}").contains("activation marker is incomplete"), "{stdout}\n{stderr}");
    assert_eq!(code, 0, "index must succeed without local state\nstdout:\n{stdout}\nstderr:\n{stderr}");

    let (stdout, stderr, code) = run(&repo, &["source", "list", "--project", "alpha"], &[]);
    assert!(!format!("{stdout}{stderr}").contains("activation marker is incomplete"), "{stdout}\n{stderr}");
    assert_eq!(code, 0, "list must succeed without local state\nstdout:\n{stdout}\nstderr:\n{stderr}");
}
