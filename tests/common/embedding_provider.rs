//! Test support for the OpenAI-compatible `/v1/embeddings` provider contract:
//! a recording mock server, repo setup with the Lance backend enabled, and
//! manifest patch helpers. Compiled into every test binary declaring
//! `mod common`, so unused items are expected per binary.
#![allow(dead_code)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::{Arc, Mutex};

use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;

/// Environment variable tests use for the provider credential. `run` always
/// removes it from the child environment unless a test passes it explicitly.
pub const KEY_ENV: &str = "MF_TEST_EMBEDDING_API_KEY";
/// Deliberately lowercase so case-insensitive header scans stay simple.
pub const SECRET: &str = "sk-test-secret-2f7c41ab";
pub const MODEL: &str = "test-embedding-model";

const MANIFEST: &str = "schema_version: '1'\nprojects:\n  \
    - name: alpha\n    path: ./projects/alpha\n    created_at: \"2026-07-17T08:00:00Z\"\n    archived_at: ~\n";

/// Build a repo with one indexed Markdown Source in project alpha and the
/// Lance backend enabled, ready for provider-backed sync/search.
pub fn provider_repo() -> TempDir {
    let repo = TempDir::new().expect("temp repo");
    std::fs::write(repo.path().join("minds.yaml"), MANIFEST).expect("write manifest");
    let project = repo.path().join("projects/alpha");
    std::fs::create_dir_all(project.join("sources/file")).expect("source dir");
    std::fs::write(project.join("mind.yaml"), "schema_version: '1'\n").expect("mind.yaml");
    std::fs::write(
        project.join("sources/file/notes.md"),
        "# Notes\n\nQuantum entanglement enables teleportation of state.\n",
    )
    .expect("source file");
    let (stdout, stderr, code) = run(&repo, &["source", "index", "--project", "alpha"], &[]);
    assert_eq!(code, 0, "source index failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let (stdout, stderr, code) = run(&repo, &["source", "advanced", "enable"], &[]);
    assert_eq!(code, 0, "advanced enable failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    repo
}

/// Run `mf --output json` in the repo and return (stdout, stderr, exit code).
/// The credential env var is present only when passed via `envs`, so each test
/// controls credential resolution deterministically.
pub fn run(repo: &TempDir, args: &[&str], envs: &[(&str, &str)]) -> (String, String, i32) {
    let mut cmd = Command::cargo_bin("mf").expect("mf binary");
    cmd.arg("--root").arg(repo.path()).args(["--output", "json"]).args(args).env_remove(KEY_ENV);
    for (name, value) in envs {
        cmd.env(name, value);
    }
    let output = cmd.output().expect("run mf");
    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status.code().unwrap_or(-1),
    )
}

/// Parse the inner command report from the `mf --output json` envelope.
pub fn report(stdout: &str) -> Value {
    let envelope: Value =
        serde_json::from_str(stdout).unwrap_or_else(|e| panic!("stdout must be pure JSON: {e}\n{stdout}"));
    envelope["data"]["data"].clone()
}

/// Merge the given keys into `source.advanced` in `minds.yaml`, preserving
/// everything else in the manifest.
pub fn configure_embedding(root: &Path, entries: &[(&str, serde_yaml::Value)]) {
    let path = root.join("minds.yaml");
    let text = std::fs::read_to_string(&path).expect("read minds.yaml");
    let mut manifest: serde_yaml::Value = serde_yaml::from_str(&text).expect("parse minds.yaml");
    {
        let root_map = manifest.as_mapping_mut().expect("manifest mapping");
        let source = ensure_mapping(root_map, "source");
        let advanced = ensure_mapping(source, "advanced");
        for (key, value) in entries {
            advanced.insert(serde_yaml::Value::from(*key), value.clone());
        }
    }
    std::fs::write(&path, serde_yaml::to_string(&manifest).expect("serialize minds.yaml")).expect("write minds.yaml");
}

/// Configure a complete provider: endpoint, model, and credential env var.
pub fn configure_provider(root: &Path, endpoint: &str) {
    configure_embedding(
        root,
        &[
            ("embedding_endpoint", endpoint.into()),
            ("embedding_model", MODEL.into()),
            ("embedding_api_key_env", KEY_ENV.into()),
        ],
    );
}

fn ensure_mapping<'a>(map: &'a mut serde_yaml::Mapping, key: &str) -> &'a mut serde_yaml::Mapping {
    let key = serde_yaml::Value::from(key);
    if !map.contains_key(&key) {
        map.insert(key.clone(), serde_yaml::Value::Mapping(serde_yaml::Mapping::new()));
    }
    map.get_mut(&key).and_then(|value| value.as_mapping_mut()).expect("mapping value")
}

/// How the mock provider answers each request.
#[derive(Clone, Copy)]
pub enum Behavior {
    /// Valid response: one vector of the given dimension per input.
    Vectors(usize),
    /// HTTP error status with an empty JSON object body.
    HttpError(u16),
    /// Count- and dimension-correct vectors containing non-finite values.
    NonFinite(usize),
    /// Read the request, then stall for the given seconds without replying.
    Hang(u64),
    /// Deterministic concept embedding: related text lands nearby, unrelated
    /// text is orthogonal. Lets a search test prove the provider-backed vector
    /// path end to end without a real model.
    Semantic(usize),
}

pub struct RecordedRequest {
    pub headers: String,
    pub body: String,
}

/// Minimal recording HTTP server standing in for an OpenAI-compatible
/// `/v1/embeddings` provider on loopback.
pub struct MockProvider {
    pub endpoint: String,
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
}

impl MockProvider {
    pub fn start(behavior: Behavior) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock provider");
        let endpoint = format!("http://{}", listener.local_addr().expect("mock provider address"));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&requests);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                let Some((headers, body)) = read_request(&mut stream) else { continue };
                let inputs: Vec<String> = serde_json::from_str::<Value>(&body)
                    .ok()
                    .and_then(|request| {
                        request["input"].as_array().map(|inputs| {
                            inputs.iter().filter_map(|value| value.as_str().map(str::to_string)).collect()
                        })
                    })
                    .unwrap_or_default();
                let input_count = inputs.len();
                recorded.lock().expect("mock lock").push(RecordedRequest { headers, body });
                match behavior {
                    Behavior::Vectors(dimension) => {
                        respond(&mut stream, 200, &vectors_body(input_count, dimension, false))
                    }
                    Behavior::NonFinite(dimension) => {
                        respond(&mut stream, 200, &vectors_body(input_count, dimension, true))
                    }
                    Behavior::Semantic(dimension) => respond(&mut stream, 200, &semantic_body(&inputs, dimension)),
                    Behavior::HttpError(status) => respond(&mut stream, status, "{}"),
                    Behavior::Hang(seconds) => std::thread::sleep(std::time::Duration::from_secs(seconds)),
                }
            }
        });
        Self { endpoint, requests }
    }

    pub fn requests(&self) -> Vec<RecordedRequest> {
        self.requests
            .lock()
            .expect("mock lock")
            .iter()
            .map(|request| RecordedRequest { headers: request.headers.clone(), body: request.body.clone() })
            .collect()
    }

    pub fn request_count(&self) -> usize {
        self.requests.lock().expect("mock lock").len()
    }
}

/// Read one HTTP request from the stream, returning (headers, body). Shared
/// with the mock content site in the ingestion tests.
pub fn read_request(stream: &mut TcpStream) -> Option<(String, String)> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 4096];
    let header_end = loop {
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            return None;
        }
        buffer.extend_from_slice(&chunk[..n]);
        if let Some(position) = find(&buffer, b"\r\n\r\n") {
            break position + 4;
        }
    };
    let headers = String::from_utf8_lossy(&buffer[..header_end]).to_string();
    let content_length = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") { value.trim().parse::<usize>().ok() } else { None }
        })
        .unwrap_or(0);
    while buffer.len() < header_end + content_length {
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..n]);
    }
    let body_end = (header_end + content_length).min(buffer.len());
    let body = String::from_utf8_lossy(&buffer[header_end..body_end]).to_string();
    Some((headers, body))
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}

fn respond(stream: &mut TcpStream, status: u16, body: &str) {
    let response = format!(
        "HTTP/1.1 {status} Mock\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

fn vectors_body(count: usize, dimension: usize, non_finite: bool) -> String {
    let value = if non_finite { "1e400" } else { "0.1" };
    let vector = vec![value; dimension].join(",");
    let items = (0..count)
        .map(|index| format!("{{\"object\":\"embedding\",\"index\":{index},\"embedding\":[{vector}]}}"))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{\"object\":\"list\",\"data\":[{items}]}}")
}

fn semantic_body(inputs: &[String], dimension: usize) -> String {
    let items = inputs
        .iter()
        .enumerate()
        .map(|(index, text)| {
            let vector =
                semantic_vector(text, dimension).iter().map(|value| value.to_string()).collect::<Vec<_>>().join(",");
            format!("{{\"object\":\"embedding\",\"index\":{index},\"embedding\":[{vector}]}}")
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{\"object\":\"list\",\"data\":[{items}]}}")
}

/// Deterministic concept embedding. A small synonym→concept dictionary maps
/// related surface terms (even with no shared tokens) onto the same basis
/// dimension, so a paraphrased query is cosine-close to the intended passage
/// and orthogonal to unrelated content. Real paraphrase semantics are faked —
/// this exists only to exercise the provider-backed vector path deterministically.
fn semantic_vector(text: &str, dimension: usize) -> Vec<f32> {
    let mut vector = vec![0_f32; dimension];
    for token in text.to_lowercase().split(|c: char| !c.is_alphanumeric()) {
        if let Some(concept) = concept_dimension(token) {
            vector[concept] += 1.0;
        }
    }
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in &mut vector {
            *value /= norm;
        }
    } else {
        // Orthogonal to every concept so unrelated text scores ~0, never NaN.
        vector[dimension - 1] = 1.0;
    }
    vector
}

fn concept_dimension(token: &str) -> Option<usize> {
    const QUANTUM: &[&str] = &[
        "quantum",
        "entanglement",
        "entangled",
        "teleportation",
        "teleport",
        "spooky",
        "nonlocal",
        "superposition",
        "qubit",
    ];
    const BIOLOGY: &[&str] =
        &["photosynthesis", "chloroplast", "chlorophyll", "sunlight", "leaf", "plant", "solar", "calvin"];
    if QUANTUM.contains(&token) {
        Some(0)
    } else if BIOLOGY.contains(&token) {
        Some(1)
    } else {
        None
    }
}

/// Minimal loopback HTTP site serving one HTML route, for registering a Web
/// Source in provider-backed search tests. Records how many requests it served.
pub struct MockContentSite {
    base: String,
    path: String,
    requests: Arc<Mutex<usize>>,
}

impl MockContentSite {
    pub fn start(path: &str, content_type: &'static str, body: &str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock site");
        let base = format!("http://{}", listener.local_addr().expect("mock site address"));
        let route = path.to_string();
        let body = body.to_string();
        let requests = Arc::new(Mutex::new(0_usize));
        let counter = Arc::clone(&requests);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                let Some((headers, _body)) = read_request(&mut stream) else { continue };
                *counter.lock().expect("site lock") += 1;
                let request_path = headers.split_whitespace().nth(1).unwrap_or("/");
                if request_path == route {
                    respond_content(&mut stream, 200, content_type, body.as_bytes());
                } else {
                    respond_content(&mut stream, 404, "text/plain", b"not found");
                }
            }
        });
        Self { base, path: path.to_string(), requests }
    }

    /// The absolute URL of the served route.
    pub fn url(&self) -> String {
        format!("{}{}", self.base, self.path)
    }

    pub fn request_count(&self) -> usize {
        *self.requests.lock().expect("site lock")
    }
}

fn respond_content(stream: &mut TcpStream, status: u16, content_type: &str, body: &[u8]) {
    let head = format!(
        "HTTP/1.1 {status} Mock\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
}
