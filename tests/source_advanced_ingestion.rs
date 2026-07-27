//! Source ingestion contract tests (Bug B): after Bug B, `source new <url>`
//! fetches and stores a local file; `sync` only reads local content and never
//! touches the network.  HTTP acquisition behaviour (bounded fetch, error
//! rejection, credential redaction, timeout, redirect/byte limits) is tested at
//! `source new` time.  Offline sync is always network-free.

use std::collections::HashMap;
use std::io::Write;
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;
use tempfile::TempDir;

mod common;
use common::embedding_provider::{configure_embedding, provider_repo, read_request, report, run};

const PAGE_HTML: &str = include_str!("fixtures/source_advanced/page.html");
const FEED_XML: &str = include_str!("fixtures/source_advanced/feed.xml");

#[test]
fn web_html_is_ingested_via_explicit_sync() {
    let site = MockSite::start(HashMap::from([("/page.html".to_string(), SiteResponse::ok("text/html", PAGE_HTML))]));
    let repo = provider_repo();
    register(&repo, &format!("{}/page.html", site.base), "webpage", None);

    let (stdout, stderr, code) = run(&repo, &["source", "sync"], &[]);
    assert_eq!(code, 0, "sync failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let item = item(&report(&stdout), "webpage");
    assert_eq!(item["action"], "added", "{stdout}");
    assert!(item["affected_chunks"].as_u64().unwrap_or(0) >= 1, "{stdout}");

    let (stdout, stderr, code) = run(&repo, &["source", "search", "photon", "--mode", "advanced"], &[]);
    assert_eq!(code, 0, "search failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(
        !report(&stdout)["results"].as_array().expect("results").is_empty(),
        "fetched HTML content must be searchable\n{stdout}"
    );
}

#[test]
fn rss_feeds_are_ingested_via_explicit_sync() {
    let site =
        MockSite::start(HashMap::from([("/feed.xml".to_string(), SiteResponse::ok("application/rss+xml", FEED_XML))]));
    let repo = provider_repo();
    register(&repo, &format!("{}/feed.xml", site.base), "feed", Some("rss"));

    let (stdout, stderr, code) = run(&repo, &["source", "sync"], &[]);
    assert_eq!(code, 0, "sync failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let item = item(&report(&stdout), "feed");
    assert_eq!(item["action"], "added", "{stdout}");
    assert!(item["affected_chunks"].as_u64().unwrap_or(0) >= 1, "{stdout}");

    let (stdout, stderr, code) = run(&repo, &["source", "search", "bleaching", "--mode", "advanced"], &[]);
    assert_eq!(code, 0, "search failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(
        !report(&stdout)["results"].as_array().expect("results").is_empty(),
        "fetched RSS content must be searchable\n{stdout}"
    );
}

// ── Bug B: sync is always network-free ──

#[test]
fn offline_sync_processes_web_sources_from_local_file_without_network() {
    let site = MockSite::start(HashMap::from([("/page.html".to_string(), SiteResponse::ok("text/html", PAGE_HTML))]));
    let repo = provider_repo();
    // `source new` fetches the URL and stores a local file.
    register(&repo, &format!("{}/page.html", site.base), "webpage", None);
    let fetch_count = site.request_count();
    assert_eq!(fetch_count, 1, "`source new` must issue exactly one HTTP request; got {fetch_count}");

    // `sync --offline` reads the local file; it MUST NOT issue any network
    // request because the content was already captured by `source new`.
    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "offline sync failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let sync = report(&stdout);
    assert_eq!(sync["registrations_failed"], 0, "no item may fail\n{stdout}");
    // The web source was already registered and its local file is available, so
    // sync should process it (action "added"), not skip it.
    let webpage = item(&sync, "webpage");
    assert_eq!(webpage["action"], "added", "web source must be added from local file\n{sync}");
    // After `source new`, no further network request was made.
    assert_eq!(site.request_count(), 1, "sync must not issue any network request (Bug B)");
}

// ── Bug B: HTTP acquisition errors surface at `source new` time ──

#[test]
fn error_pages_are_rejected_at_source_add_time() {
    let site =
        MockSite::start(HashMap::from([("/page.html".to_string(), SiteResponse::status(404, "text/html", PAGE_HTML))]));
    let repo = provider_repo();

    // `source new` must fail cleanly when the server returns a non-2xx status.
    let (stdout, stderr, code) = run(
        &repo,
        &["source", "new", &format!("{}/page.html", site.base), "--project", "alpha", "--name", "webpage"],
        &[],
    );
    assert_ne!(code, 0, "source new with a 404 URL must fail\nstdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(
        stderr.contains("404") || stderr.contains("failed"),
        "error must mention the non-success status\nstderr:\n{stderr}"
    );
}

#[test]
fn https_mismatch_is_rejected_at_source_add_time() {
    // Plain-HTTP mock reached over https:// — the TLS handshake must fail as
    // a per-item error, never a crash or a fallback to cleartext.
    let site = MockSite::start(HashMap::from([("/page.html".to_string(), SiteResponse::ok("text/html", PAGE_HTML))]));
    let https_url = format!("https://{}/page.html", site.base.trim_start_matches("http://"));
    let repo = provider_repo();

    let (stdout, stderr, code) =
        run(&repo, &["source", "new", &https_url, "--project", "alpha", "--name", "webpage"], &[]);
    assert_ne!(code, 0, "source new with HTTPS-to-HTTP must fail\nstdout:\n{stdout}\nstderr:\n{stderr}");
}

#[test]
fn url_credentials_never_appear_in_reports() {
    const PASSWORD: &str = "hunter2xyz";
    let site = MockSite::start(HashMap::from([("/page.html".to_string(), SiteResponse::ok("text/html", PAGE_HTML))]));
    let with_credentials = format!("http://alice:{PASSWORD}@{}/page.html", site.base.trim_start_matches("http://"));
    let repo = provider_repo();
    register(&repo, &with_credentials, "webpage", None);

    let (stdout, stderr, code) = run(&repo, &["source", "sync"], &[]);
    assert_eq!(code, 0, "sync failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    assert_eq!(item(&report(&stdout), "webpage")["action"], "added", "{stdout}");
    assert!(!format!("{stdout}{stderr}").contains(PASSWORD), "sync leaked URL credentials");

    let (stdout, stderr, code) = run(&repo, &["source", "search", "photon", "--mode", "advanced"], &[]);
    assert_eq!(code, 0, "search failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(!format!("{stdout}{stderr}").contains(PASSWORD), "search leaked URL credentials\n{stdout}");
}

#[test]
fn redirects_are_limited_at_source_add_time() {
    let site = MockSite::start(HashMap::from([
        ("/page.html".to_string(), SiteResponse::redirect("/final.html")),
        ("/final.html".to_string(), SiteResponse::ok("text/html", PAGE_HTML)),
    ]));
    let repo = provider_repo();
    configure_embedding(repo.path(), &[("fetch_max_redirects", serde_yaml::Value::from(0_u64))]);

    let (stdout, stderr, code) = run(
        &repo,
        &["source", "new", &format!("{}/page.html", site.base), "--project", "alpha", "--name", "webpage"],
        &[],
    );
    assert_ne!(code, 0, "source new with redirect limit 0 must fail\nstdout:\n{stdout}\nstderr:\n{stderr}");
}

#[test]
fn responses_are_bounded_at_source_add_time() {
    let big = "x".repeat(4096);
    let site = MockSite::start(HashMap::from([("/page.html".to_string(), SiteResponse::ok("text/html", &big))]));
    let repo = provider_repo();
    configure_embedding(repo.path(), &[("fetch_max_bytes", serde_yaml::Value::from(64_u64))]);

    let (stdout, stderr, code) = run(
        &repo,
        &["source", "new", &format!("{}/page.html", site.base), "--project", "alpha", "--name", "webpage"],
        &[],
    );
    assert_ne!(
        code, 0,
        "source new with byte limit 64 must reject oversized response\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("byte") || stderr.contains("limit"),
        "error must mention the byte limit\nstderr:\n{stderr}"
    );
}

#[test]
fn requests_are_bounded_at_source_add_time() {
    let site = MockSite::start(HashMap::from([("/page.html".to_string(), SiteResponse::hang(30))]));
    let repo = provider_repo();
    configure_embedding(repo.path(), &[("fetch_timeout_seconds", serde_yaml::Value::from(1_u64))]);

    let started = Instant::now();
    let (stdout, stderr, code) = run(
        &repo,
        &["source", "new", &format!("{}/page.html", site.base), "--project", "alpha", "--name", "webpage"],
        &[],
    );
    let elapsed = started.elapsed();
    assert!(elapsed < Duration::from_secs(15), "source new must respect the configured timeout; took {elapsed:?}");
    assert_ne!(code, 0, "source new with timeout must fail\nstdout:\n{stdout}\nstderr:\n{stderr}");
}

/// Register a URL Source in project alpha; `kind` of `Some("rss")` registers
/// a feed, `None` a web page.
fn register(repo: &TempDir, url: &str, name: &str, kind: Option<&str>) {
    let mut args = vec!["source", "new", url, "--project", "alpha", "--name", name];
    if let Some(kind) = kind {
        args.extend(["--file-kind", kind]);
    }
    let (stdout, stderr, code) = run(repo, &args, &[]);
    assert_eq!(code, 0, "source new {url} failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
}

/// Find one sync item by source identity.
fn item(report: &Value, source_identity: &str) -> Value {
    report["items"]
        .as_array()
        .and_then(|items| items.iter().find(|item| item["source_identity"] == source_identity))
        .unwrap_or_else(|| panic!("sync item {source_identity} missing from report\n{report}"))
        .clone()
}

/// One route's canned response.
struct SiteResponse {
    status: u16,
    content_type: &'static str,
    location: Option<String>,
    body: Vec<u8>,
    hang_seconds: u64,
}

impl SiteResponse {
    fn ok(content_type: &'static str, body: &str) -> Self {
        Self { status: 200, content_type, location: None, body: body.as_bytes().to_vec(), hang_seconds: 0 }
    }

    fn status(status: u16, content_type: &'static str, body: &str) -> Self {
        Self { status, content_type, location: None, body: body.as_bytes().to_vec(), hang_seconds: 0 }
    }

    fn redirect(to: &str) -> Self {
        Self {
            status: 302,
            content_type: "text/html",
            location: Some(to.to_string()),
            body: Vec::new(),
            hang_seconds: 0,
        }
    }

    fn hang(seconds: u64) -> Self {
        Self { status: 200, content_type: "text/html", location: None, body: Vec::new(), hang_seconds: seconds }
    }
}

/// Minimal routed HTTP server for registered Source URLs on loopback.
struct MockSite {
    base: String,
    requests: Arc<Mutex<Vec<String>>>,
}

impl MockSite {
    fn start(routes: HashMap<String, SiteResponse>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock site");
        let base = format!("http://{}", listener.local_addr().expect("mock site address"));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&requests);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                let Some((headers, _body)) = read_request(&mut stream) else { continue };
                let path = headers.split_whitespace().nth(1).unwrap_or("/").to_string();
                recorded.lock().expect("site lock").push(path.clone());
                let Some(route) = routes.get(&path) else {
                    respond(&mut stream, 404, "text/plain", None, b"not found");
                    continue;
                };
                if route.hang_seconds > 0 {
                    std::thread::sleep(Duration::from_secs(route.hang_seconds));
                    continue;
                }
                respond(&mut stream, route.status, route.content_type, route.location.as_deref(), &route.body);
            }
        });
        Self { base, requests }
    }

    fn request_count(&self) -> usize {
        self.requests.lock().expect("site lock").len()
    }
}

fn respond(stream: &mut std::net::TcpStream, status: u16, content_type: &str, location: Option<&str>, body: &[u8]) {
    let location_header = location.map(|to| format!("Location: {to}\r\n")).unwrap_or_default();
    let head = format!(
        "HTTP/1.1 {status} Mock\r\nContent-Type: {content_type}\r\n{location_header}Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
}
