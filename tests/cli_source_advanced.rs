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

    let (stdout, stderr, code) = run(&repo, &["source", "advanced", "sync"], &[]);
    assert_eq!(code, 2, "endpoint without model must be a usage error\nstdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(
        format!("{stdout}{stderr}").contains("embedding_model"),
        "error must name the missing key\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[test]
fn advanced_search_degrades_deterministically_without_a_provider() {
    let repo = provider_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "advanced", "sync", "--offline"], &[]);
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
    let (stdout, stderr, code) = run(&repo, &["source", "advanced", "sync", "--offline"], &[]);
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
    let (stdout, stderr, code) = run(&repo, &["source", "advanced", "sync", "--offline"], &[]);
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

    let (stdout, stderr, code) = run(&repo, &["source", "advanced", "sync", "--offline"], &[(KEY_ENV, SECRET)]);
    assert_eq!(code, 0, "offline sync failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    assert_eq!(
        mock.request_count(),
        0,
        "--offline must not contact the embedding provider\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}
