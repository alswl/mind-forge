//! Integration tests and helpers for `mf init` and repo lifecycle commands.

use assert_cmd::Command;
use std::path::Path;

/// Build the `mf` binary and return a `Command`.
fn mf() -> Command {
    Command::cargo_bin("mf").unwrap()
}

// ── Helper functions ──

/// Parse the JSON envelope from `mf --json` stdout.
///
/// Returns the parsed `data` field for a successful command, or panics.
pub fn parse_json_envelope(stdout: &[u8]) -> serde_json::Value {
    let val: serde_json::Value = serde_json::from_slice(stdout).expect("stdout must be valid JSON");
    assert_eq!(val["status"].as_str(), Some("ok"), "envelope status must be ok");
    val["data"].clone()
}

/// Run `mf` with the given args in `dir` and return the parsed JSON data envelope.
pub fn run_json(dir: &Path, args: &[&str]) -> serde_json::Value {
    let output = mf().current_dir(dir).args(args).arg("--json").output().expect("mf must run");
    assert!(output.status.success(), "mf should succeed: stderr={}", String::from_utf8_lossy(&output.stderr));
    parse_json_envelope(&output.stdout)
}

/// Run `mf` with the given args in `dir`, expecting failure, and return stderr and exit code.
pub fn run_failure(dir: &Path, args: &[&str]) -> (String, i32) {
    let output = mf().current_dir(dir).args(args).output().expect("mf must run");
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or_default();
    (stderr, code)
}

/// Assert that a directory is a valid Mind Repo (has `minds.yaml` and `projects/`).
pub fn assert_mind_repo_root(dir: &Path) {
    assert!(dir.join("minds.yaml").exists(), "minds.yaml missing in {}", dir.display());
    assert!(dir.join("projects").is_dir(), "projects/ missing in {}", dir.display());
}

/// Assert that `path` is NOT a valid Mind Repo (no `minds.yaml` created).
pub fn assert_not_mind_repo(dir: &Path) {
    assert!(!dir.join("minds.yaml").exists(), "unexpected minds.yaml found in {}", dir.display());
}

// ── T011: mf init creates minds.yaml and projects/ in current directory ──

#[test]
fn test_init_current_dir_creates_repo() {
    let dir = tempfile::tempdir().unwrap();
    mf().current_dir(dir.path()).arg("init").assert().success();
    assert_mind_repo_root(dir.path());
}

// ── T012: JSON envelope test for mf init --json ──

#[test]
fn test_init_current_dir_json_envelope() {
    let dir = tempfile::tempdir().unwrap();
    let data = run_json(dir.path(), &["init"]);
    assert_eq!(data["created"].as_bool(), Some(true));
    assert_eq!(data["already_existed"].as_bool(), Some(false));
    let created_files: Vec<&str> =
        data["created_files"].as_array().expect("created_files array").iter().map(|v| v.as_str().unwrap()).collect();
    assert!(created_files.contains(&"minds.yaml"));
    let created_dirs: Vec<&str> = data["created_directories"]
        .as_array()
        .expect("created_directories array")
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(created_dirs.contains(&"projects"));
    assert!(data["skipped"].as_array().unwrap().is_empty());
}

// ── T013: Idempotency — repeated mf init is non-destructive ──

#[test]
fn test_init_idempotent() {
    let dir = tempfile::tempdir().unwrap();

    // First init — creates the repo.
    let data = run_json(dir.path(), &["init"]);
    assert_eq!(data["created"].as_bool(), Some(true));

    // Read minds.yaml content after first init.
    let minds_content = std::fs::read_to_string(dir.path().join("minds.yaml")).unwrap();

    // Second init — reports already_existed, doesn't rewrite.
    let data2 = run_json(dir.path(), &["init"]);
    assert_eq!(data2["created"].as_bool(), Some(false));
    assert_eq!(data2["already_existed"].as_bool(), Some(true));
    assert!(data2["created_files"].as_array().unwrap().is_empty());
    assert!(data2["skipped"].as_array().unwrap().contains(&serde_json::json!("minds.yaml")));

    // Verify minds.yaml was not rewritten.
    let minds_content2 = std::fs::read_to_string(dir.path().join("minds.yaml")).unwrap();
    assert_eq!(minds_content, minds_content2);
}

// ── Phase 4: US2 — Initialize a target path ──

// T020: mf init <missing-dir> creates and initializes the target
#[test]
fn test_init_missing_target_creates_and_inits() {
    let parent = tempfile::tempdir().unwrap();
    let target = parent.path().join("my-notes");
    let data = run_json(parent.path(), &["init", "my-notes"]);
    assert_eq!(data["created"].as_bool(), Some(true));
    assert!(target.join("minds.yaml").exists());
    assert!(target.join("projects").is_dir());
}

// T021: mf init <existing-empty> initializes the target
#[test]
fn test_init_existing_empty_target() {
    let parent = tempfile::tempdir().unwrap();
    let target = parent.path().join("existing");
    std::fs::create_dir(&target).unwrap();
    let data = run_json(parent.path(), &["init", "existing"]);
    assert_eq!(data["created"].as_bool(), Some(true));
    assert_mind_repo_root(&target);
}

// T022: JSON envelope for target-path init
#[test]
fn test_init_target_path_json_envelope() {
    let parent = tempfile::tempdir().unwrap();
    let data = run_json(parent.path(), &["init", "json-notes"]);
    assert_eq!(data["created"].as_bool(), Some(true));
    assert_eq!(data["already_existed"].as_bool(), Some(false));
    assert!(data["path"].as_str().unwrap().contains("json-notes"));
    let created_files: Vec<&str> =
        data["created_files"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    assert!(created_files.contains(&"minds.yaml"));
}

// ── Phase 5: US3 — Avoid accidental overwrites ──

// T026: Refuse non-empty non-repo target
#[test]
fn test_init_refuses_non_empty_target() {
    let parent = tempfile::tempdir().unwrap();
    let target = parent.path().join("unsafe");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("readme.txt"), "notes").unwrap();
    let (stderr, code) = run_failure(parent.path(), &["init", "unsafe"]);
    assert_eq!(code, 2);
    assert!(stderr.contains("not empty"));
    // Verify files unchanged.
    assert!(target.join("readme.txt").exists());
    assert!(!target.join("minds.yaml").exists());
}

// T027: Refuse malformed minds.yaml without overwrite
#[test]
fn test_init_refuses_malformed_manifest() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("minds.yaml"), "invalid: yaml: [[[").unwrap();
    let (stderr, code) = run_failure(dir.path(), &["init"]);
    assert_eq!(code, 2);
    assert!(stderr.contains("malformed") || stderr.contains("incompatible"));
    // Verify malformed manifest is unchanged.
    let content = std::fs::read_to_string(dir.path().join("minds.yaml")).unwrap();
    assert_eq!(content, "invalid: yaml: [[[");
}

// T028: Refuse file target, missing parent, path traversal
#[test]
fn test_init_refuses_file_target() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("a-file.txt");
    std::fs::write(&file_path, "content").unwrap();
    let (stderr, code) = run_failure(dir.path(), &["init", "a-file.txt"]);
    assert_eq!(code, 2);
    assert!(stderr.contains("file"));
}

#[test]
fn test_init_refuses_missing_parent() {
    let dir = tempfile::tempdir().unwrap();
    let (stderr, code) = run_failure(dir.path(), &["init", "missing-parent/child"]);
    assert_ne!(code, 0);
    assert!(stderr.contains("parent"));
}

#[test]
fn test_init_refuses_dotdot() {
    let dir = tempfile::tempdir().unwrap();
    let (_stderr, code) = run_failure(dir.path(), &["init", ".."]);
    assert_ne!(code, 0);
}

// T029: Refuse nested repo init
#[test]
fn test_init_refuses_nested_repo() {
    let dir = tempfile::tempdir().unwrap();
    // Create a parent repo.
    std::fs::write(dir.path().join("minds.yaml"), "schema: '1'\nprojects: []\n").unwrap();
    // Try to init a subdirectory inside the repo.
    let sub = dir.path().join("subdir");
    std::fs::create_dir(&sub).unwrap();
    let (stderr, code) = run_failure(dir.path(), &["init", "subdir"]);
    assert_eq!(code, 2);
    assert!(stderr.contains("inside an existing Mind Repo"));
}
