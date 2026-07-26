//! Provider-contract tests for the OpenAI-compatible `/v1/embeddings`
//! integration (T089): explicit configuration, environment-only credential
//! resolution, redacted diagnostics, bounded timeouts, and dimension /
//! finite-vector validation.

use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::Value;

mod common;
use common::embedding_provider::{
    Behavior, KEY_ENV, MODEL, MockContentSite, MockProvider, SECRET, configure_embedding, configure_provider,
    provider_repo, report, run,
};
use tempfile::TempDir;

/// minds.yaml registering two active projects, alpha and beta.
const MANIFEST_TWO_PROJECT: &str = "schema_version: '1'\nprojects:\n  \
    - name: alpha\n    path: ./projects/alpha\n    created_at: \"2026-07-17T08:00:00Z\"\n    archived_at: ~\n  \
    - name: beta\n    path: ./projects/beta\n    created_at: \"2026-07-17T09:00:00Z\"\n    archived_at: ~\n";

const BIO_HTML: &str = "<html><body><h1>Biology</h1><p>Photosynthesis in the chloroplast converts sunlight into chemical energy.</p></body></html>";

/// Two-project repo with the Lance backend enabled and a provider configured:
/// alpha owns a local Markdown Source (quantum concept), beta owns a Web Source
/// served from `web_url` (biology concept). Content is not synced yet.
fn semantic_two_project_repo(embeddings_endpoint: &str, web_url: &str) -> TempDir {
    let repo = TempDir::new().expect("temp repo");
    std::fs::write(repo.path().join("minds.yaml"), MANIFEST_TWO_PROJECT).expect("write manifest");

    let alpha = repo.path().join("projects/alpha");
    std::fs::create_dir_all(alpha.join("sources/file")).expect("alpha source dir");
    std::fs::write(alpha.join("mind.yaml"), "schema_version: '1'\n").expect("alpha mind.yaml");
    std::fs::write(
        alpha.join("sources/file/notes.md"),
        "# Alpha Notes\n\nQuantum entanglement enables teleportation of state.\n",
    )
    .expect("alpha source");

    let beta = repo.path().join("projects/beta");
    std::fs::create_dir_all(&beta).expect("beta dir");
    std::fs::write(beta.join("mind.yaml"), "schema_version: '1'\n").expect("beta mind.yaml");

    let (out, err, code) = run(&repo, &["source", "index", "--project", "alpha"], &[]);
    assert_eq!(code, 0, "index alpha failed\nstdout:\n{out}\nstderr:\n{err}");
    let (out, err, code) = run(&repo, &["source", "advanced", "enable"], &[]);
    assert_eq!(code, 0, "enable failed\nstdout:\n{out}\nstderr:\n{err}");
    configure_provider(repo.path(), embeddings_endpoint);
    // Register beta's Web Source after activation so the URL is stored in the
    // Lance primary catalog (matches the proven Lance-mode `source new` path).
    let (out, err, code) = run(&repo, &["source", "new", web_url, "--project", "beta", "--name", "bio-web"], &[]);
    assert_eq!(code, 0, "register beta web source failed\nstdout:\n{out}\nstderr:\n{err}");
    repo
}

/// First registration whose registered_location matches `needle`.
fn registration_location(result: &serde_json::Value, needle: &str) -> bool {
    result["registrations"]
        .as_array()
        .map(|regs| regs.iter().any(|r| r["registered_location"].as_str().unwrap_or_default().contains(needle)))
        .unwrap_or(false)
}

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

// ── T093: two-project provider-backed semantic search ──

#[test]
fn advanced_semantic_hits_local_and_http_sources_across_projects() {
    let embeddings = MockProvider::start(Behavior::Semantic(384));
    let site = MockContentSite::start("/bio.html", "text/html", BIO_HTML);
    let repo = semantic_two_project_repo(&embeddings.endpoint, &site.url());

    let (stdout, stderr, code) = run(&repo, &["source", "advanced", "sync"], &[(KEY_ENV, SECRET)]);
    assert_eq!(code, 0, "provider-backed sync failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let sync = report(&stdout);
    assert_eq!(sync["registrations_failed"], 0, "no registration may fail\n{stdout}");
    assert!(sync["registrations_added"].as_u64().unwrap_or(0) >= 2, "both projects embedded\n{stdout}");
    assert!(site.request_count() >= 1, "beta's Web Source must be fetched during sync");

    // Local (alpha): the query shares no tokens with the source text, so only
    // the provider-backed vector path can surface it.
    let (stdout, stderr, code) =
        run(&repo, &["source", "search", "spooky nonlocal superposition", "--mode", "advanced"], &[(KEY_ENV, SECRET)]);
    assert_eq!(code, 0, "alpha semantic search failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let alpha = report(&stdout);
    let results = alpha["results"].as_array().expect("results array");
    assert!(!results.is_empty(), "the quantum paraphrase must match alpha via the vector path\n{stdout}");
    assert!(
        registration_location(&results[0], "sources/file/notes.md"),
        "top hit resolves to alpha's source\n{stdout}"
    );
    assert!(
        alpha["actual_paths"].as_array().expect("paths").iter().any(|p| p == "advanced_hybrid"),
        "the provider-backed hybrid path must be used\n{stdout}"
    );

    // HTTP (beta): a paraphrase with no shared tokens matches fetched HTML.
    let (stdout, stderr, code) =
        run(&repo, &["source", "search", "chlorophyll solar leaf", "--mode", "advanced"], &[(KEY_ENV, SECRET)]);
    assert_eq!(code, 0, "beta semantic search failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let beta = report(&stdout);
    let results = beta["results"].as_array().expect("results array");
    assert!(!results.is_empty(), "the biology paraphrase must match beta via the vector path\n{stdout}");
    assert!(
        results[0]["snippet"].as_str().unwrap_or_default().to_lowercase().contains("photosynthesis"),
        "beta's top hit carries fetched HTML content\n{stdout}"
    );

    // The provider credential never lands on disk.
    let mut hits = Vec::new();
    scan_for_secret(repo.path(), SECRET.as_bytes(), &mut hits);
    assert!(hits.is_empty(), "credential persisted on disk at {hits:?}");
}

#[test]
fn both_mode_fuses_provider_results_with_provenance() {
    let embeddings = MockProvider::start(Behavior::Semantic(384));
    let site = MockContentSite::start("/bio.html", "text/html", BIO_HTML);
    let repo = semantic_two_project_repo(&embeddings.endpoint, &site.url());
    let (out, err, code) = run(&repo, &["source", "advanced", "sync"], &[(KEY_ENV, SECRET)]);
    assert_eq!(code, 0, "sync failed\nstdout:\n{out}\nstderr:\n{err}");

    let (stdout, stderr, code) = run(&repo, &["source", "search", "quantum", "--mode", "both"], &[(KEY_ENV, SECRET)]);
    assert_eq!(code, 0, "both-mode search failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let both = report(&stdout);
    assert_eq!(both["degraded"], false, "healthy provider must not degrade both-mode\n{stdout}");
    let results = both["results"].as_array().expect("results array");
    assert!(!results.is_empty(), "both-mode returns the fused hit\n{stdout}");
    assert!(registration_location(&results[0], "sources/file/notes.md"), "provenance resolves to alpha\n{stdout}");
    assert!(
        both["actual_paths"].as_array().expect("paths").iter().any(|p| p == "advanced_hybrid"),
        "both-mode reports the advanced retrieval path\n{stdout}"
    );
}

#[test]
fn advanced_degrades_when_the_provider_is_unreachable_but_still_returns_keyword_hits() {
    let embeddings = MockProvider::start(Behavior::Semantic(384));
    let site = MockContentSite::start("/bio.html", "text/html", BIO_HTML);
    let repo = semantic_two_project_repo(&embeddings.endpoint, &site.url());
    let (out, err, code) = run(&repo, &["source", "advanced", "sync"], &[(KEY_ENV, SECRET)]);
    assert_eq!(code, 0, "sync failed\nstdout:\n{out}\nstderr:\n{err}");

    // Point the query-time provider at a closed port; sync already persisted vectors.
    configure_embedding(repo.path(), &[("embedding_endpoint", serde_yaml::Value::from("http://127.0.0.1:1"))]);

    let (stdout, stderr, code) = run(&repo, &["source", "search", "quantum", "--mode", "both"], &[(KEY_ENV, SECRET)]);
    assert_eq!(code, 0, "degraded both-mode must still succeed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let degraded = report(&stdout);
    let warnings = degraded["warnings"]
        .as_array()
        .map(|w| w.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join("\n"))
        .unwrap_or_default();
    assert!(warnings.contains("degraded"), "an unreachable provider must report degradation\n{stdout}");
    assert!(
        !degraded["results"].as_array().expect("results array").is_empty(),
        "keyword retrieval still returns the quantum hit when the provider is down\n{stdout}"
    );
    assert!(!format!("{stdout}{stderr}").contains(SECRET), "credential leaked into degraded diagnostics");
}

#[test]
fn basic_and_status_make_no_provider_or_network_calls() {
    let embeddings = MockProvider::start(Behavior::Semantic(384));
    let site = MockContentSite::start("/bio.html", "text/html", BIO_HTML);
    let repo = semantic_two_project_repo(&embeddings.endpoint, &site.url());
    let (out, err, code) = run(&repo, &["source", "advanced", "sync"], &[(KEY_ENV, SECRET)]);
    assert_eq!(code, 0, "sync failed\nstdout:\n{out}\nstderr:\n{err}");

    let embedding_calls = embeddings.request_count();
    let site_calls = site.request_count();

    // Basic search runs without the credential and must not embed or fetch.
    let (stdout, _, code) = run(&repo, &["source", "search", "bio-web", "--mode", "basic"], &[]);
    assert_eq!(code, 0, "basic search failed\n{stdout}");
    let (stdout, _, code) = run(&repo, &["source", "advanced", "status"], &[]);
    assert_eq!(code, 0, "status failed\n{stdout}");

    assert_eq!(embeddings.request_count(), embedding_calls, "basic/status must not call the embedding provider");
    assert_eq!(site.request_count(), site_calls, "basic/status must not fetch Source URLs");
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
