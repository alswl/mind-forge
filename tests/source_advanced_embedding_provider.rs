//! Provider-contract tests for the OpenAI-compatible `/v1/embeddings`
//! integration (T089): explicit configuration, environment-only credential
//! resolution, redacted diagnostics, bounded timeouts, and dimension /
//! finite-vector validation.

use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::Value;

mod common;
use common::embedding_provider::{
    Behavior, KEY_ENV, MODEL, MockProvider, SECRET, configure_embedding, configure_provider, provider_repo, report, run,
};

#[test]
fn sync_embeds_chunks_through_the_configured_provider() {
    let mock = MockProvider::start(Behavior::Vectors(384));
    let repo = provider_repo();
    configure_provider(repo.path(), &mock.endpoint);

    let (stdout, stderr, code) = run(&repo, &["source", "advanced", "sync"], &[(KEY_ENV, SECRET)]);
    assert_eq!(code, 0, "sync failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let report = report(&stdout);
    assert_eq!(report["registrations_failed"], 0, "{stdout}");
    assert!(report["registrations_added"].as_u64().unwrap_or(0) >= 1, "{stdout}");

    let requests = mock.requests();
    assert!(!requests.is_empty(), "sync must call the configured provider");
    let first = &requests[0];
    assert!(
        first.headers.to_ascii_lowercase().contains(&format!("authorization: bearer {SECRET}")),
        "provider request must carry the bearer credential resolved from {KEY_ENV}\n{}",
        first.headers
    );
    let body: Value = serde_json::from_str(&first.body).expect("provider request body must be JSON");
    assert_eq!(body["model"], MODEL, "{}", first.body);
    assert!(!body["input"].as_array().expect("input array").is_empty(), "{}", first.body);
}

#[test]
fn credentials_never_reach_disk_or_diagnostics() {
    let mock = MockProvider::start(Behavior::Vectors(384));
    let repo = provider_repo();
    configure_provider(repo.path(), &mock.endpoint);

    let (stdout, stderr, code) = run(&repo, &["source", "advanced", "sync"], &[(KEY_ENV, SECRET)]);
    assert_eq!(code, 0, "sync failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(!stdout.contains(SECRET) && !stderr.contains(SECRET), "credential leaked into command output");

    let mut hits = Vec::new();
    scan_for_secret(repo.path(), SECRET.as_bytes(), &mut hits);
    assert!(hits.is_empty(), "credential persisted on disk at {hits:?}");
    let manifest = std::fs::read_to_string(repo.path().join("minds.yaml")).expect("read minds.yaml");
    assert!(manifest.contains(KEY_ENV), "manifest must name the credential env var\n{manifest}");
}

#[test]
fn missing_credential_env_var_is_a_usage_error_before_any_request() {
    let mock = MockProvider::start(Behavior::Vectors(384));
    let repo = provider_repo();
    configure_provider(repo.path(), &mock.endpoint);

    let (stdout, stderr, code) = run(&repo, &["source", "advanced", "sync"], &[]);
    assert_eq!(code, 2, "missing credential must be a usage error\nstdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(
        format!("{stdout}{stderr}").contains(KEY_ENV),
        "error must name the missing env var\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(mock.request_count(), 0, "no provider request may be made without a credential");
}

#[test]
fn provider_http_errors_are_reported_without_leaking_the_credential() {
    let mock = MockProvider::start(Behavior::HttpError(500));
    let repo = provider_repo();
    configure_provider(repo.path(), &mock.endpoint);

    let (stdout, stderr, code) = run(&repo, &["source", "advanced", "sync"], &[(KEY_ENV, SECRET)]);
    assert_eq!(code, 0, "per-item provider failures must not abort sync\nstdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(!format!("{stdout}{stderr}").contains(SECRET), "credential leaked into diagnostics");
    let report = report(&stdout);
    assert!(report["registrations_failed"].as_u64().unwrap_or(0) >= 1, "provider failure must be visible\n{stdout}");
    assert!(item_errors(&report).contains("HTTP"), "item error must describe the provider failure\n{stdout}");
}

#[test]
fn wrong_dimension_vectors_are_rejected() {
    let mock = MockProvider::start(Behavior::Vectors(3));
    let repo = provider_repo();
    configure_provider(repo.path(), &mock.endpoint);

    let (stdout, stderr, code) = run(&repo, &["source", "advanced", "sync"], &[(KEY_ENV, SECRET)]);
    assert_eq!(code, 0, "sync run failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let report = report(&stdout);
    assert!(report["registrations_failed"].as_u64().unwrap_or(0) >= 1, "wrong dimension must fail the item\n{stdout}");
    assert!(item_errors(&report).contains("dimension"), "item error must mention the dimension check\n{stdout}");
}

#[test]
fn non_finite_vectors_are_rejected() {
    let mock = MockProvider::start(Behavior::NonFinite(384));
    let repo = provider_repo();
    configure_provider(repo.path(), &mock.endpoint);

    let (stdout, stderr, code) = run(&repo, &["source", "advanced", "sync"], &[(KEY_ENV, SECRET)]);
    assert_eq!(code, 0, "sync run failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let report = report(&stdout);
    assert!(
        report["registrations_failed"].as_u64().unwrap_or(0) >= 1,
        "non-finite vectors must fail the item\n{stdout}"
    );
    assert!(item_errors(&report).contains("embedding provider"), "item error must name the provider\n{stdout}");
}

#[test]
fn provider_requests_are_bounded_by_the_configured_timeout() {
    let mock = MockProvider::start(Behavior::Hang(30));
    let repo = provider_repo();
    configure_provider(repo.path(), &mock.endpoint);
    configure_embedding(repo.path(), &[("fetch_timeout_seconds", serde_yaml::Value::from(1_u64))]);

    let started = Instant::now();
    let (stdout, stderr, code) = run(&repo, &["source", "advanced", "sync"], &[(KEY_ENV, SECRET)]);
    let elapsed = started.elapsed();
    assert!(elapsed < Duration::from_secs(20), "sync must respect the configured timeout; took {elapsed:?}");
    assert_eq!(code, 0, "sync run failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let report = report(&stdout);
    assert!(report["registrations_failed"].as_u64().unwrap_or(0) >= 1, "timeout must fail the item\n{stdout}");
}

/// All item error strings joined, for substring assertions.
fn item_errors(report: &Value) -> String {
    report["items"]
        .as_array()
        .map(|items| items.iter().filter_map(|item| item["error"].as_str()).collect::<Vec<_>>().join("\n"))
        .unwrap_or_default()
}

fn scan_for_secret(dir: &Path, secret: &[u8], hits: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("read dir") {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            scan_for_secret(&path, secret, hits);
        } else if let Ok(bytes) = std::fs::read(&path)
            && bytes.windows(secret.len()).any(|window| window == secret)
        {
            hits.push(path);
        }
    }
}
