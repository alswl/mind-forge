//! Focused command-level tests for Mind YAML Compatibility (017-mind-yaml).
//!
//! These tests exercise individual commands against the deterministic
//! compatibility fixture repo and verify JSON envelope, exit codes, and
//! specific data shape assertions.

use std::fs;
use std::path::Path;

use assert_cmd::Command;

use common::*;

mod common;

/// Helper: copy the compatibility testdata to a temp directory.
fn setup_compat_repo() -> TempDir {
    let testdata = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/e2e/testdata/mind-yaml-compat");
    let dir = TempDir::new().expect("temp dir");
    copy_dir_recursively(&testdata, dir.path());
    dir
}

/// Helper: run mf in dir with args.
fn run_in(dir: impl AsRef<Path>, args: &[&str]) -> (String, String, i32) {
    let output = Command::cargo_bin("mf")
        .expect("mf binary")
        .current_dir(dir.as_ref())
        .args(args)
        .output()
        .expect("command runs");
    (
        String::from_utf8(output.stdout).expect("stdout utf8"),
        String::from_utf8(output.stderr).expect("stderr utf8"),
        output.status.code().unwrap_or_default(),
    )
}

/// Helper: run mf with args in dir and return parsed JSON value.
fn run_json(dir: impl AsRef<Path>, args: &[&str]) -> (serde_json::Value, String, i32) {
    let (stdout, stderr, code) = run_in(dir, args);
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap_or(serde_json::Value::Null);
    (value, stderr, code)
}

fn copy_dir_recursively(src: &Path, dst: &Path) {
    for entry in fs::read_dir(src).expect("read dir") {
        let entry = entry.expect("entry");
        let entry_type = entry.file_type().expect("file type");
        let src_path = entry.path();
        let rel = src_path.strip_prefix(src).expect("strip prefix");
        let dst_path = dst.join(rel);
        if entry_type.is_dir() {
            fs::create_dir_all(&dst_path).expect("create dir");
            copy_dir_recursively(&src_path, &dst_path);
        } else {
            fs::copy(&src_path, &dst_path).expect("copy file");
        }
    }
}

// ---------------------------------------------------------------------------
// US1: String-list root manifest
// ---------------------------------------------------------------------------

#[test]
fn cli_compat_project_list_string_manifest() {
    let dir = setup_compat_repo();
    let (value, stderr, code) = run_json(&dir, &["project", "list", "--json"]);
    assert_eq!(code, 0, "exit 0: {stderr}");
    assert_eq!(value["status"], "ok");
    let projects = value["data"]["projects"].as_array().expect("data.projects array");
    assert_eq!(projects.len(), 12, "all 12 projects");
}

// ---------------------------------------------------------------------------
// US2: Project status compatibility variants
// ---------------------------------------------------------------------------

#[test]
fn cli_compat_status_empty_mind_yaml() {
    let dir = setup_compat_repo();
    let (value, stderr, code) =
        run_json(&dir, &["project", "show", "2026-hcs-tickets", "--project", "2026-hcs-tickets", "--json"]);
    assert_eq!(code, 0, "empty mind.yaml: {stderr}");
    assert_eq!(value["status"], "ok");
}

#[test]
fn cli_compat_status_top_level_metadata() {
    let dir = setup_compat_repo();
    let (value, stderr, code) =
        run_json(&dir, &["project", "show", "2026-meetings", "--project", "2026-meetings", "--json"]);
    assert_eq!(code, 0, "top-level metadata: {stderr}");
    assert_eq!(value["status"], "ok");
    assert_eq!(value["data"]["name"], "2026-meetings");
}

#[test]
fn cli_compat_status_wrapped_project() {
    let dir = setup_compat_repo();
    let (value, stderr, code) = run_json(&dir, &["project", "show", "2026-blogs", "--project", "2026-blogs", "--json"]);
    assert_eq!(code, 0, "wrapped project: {stderr}");
    assert_eq!(value["status"], "ok");
}

#[test]
fn cli_compat_status_team_reports() {
    let dir = setup_compat_repo();
    let (value, stderr, code) =
        run_json(&dir, &["project", "show", "team-reports", "--project", "team-reports", "--json"]);
    assert_eq!(code, 0, "team-reports: {stderr}");
    assert_eq!(value["status"], "ok");
}

// ---------------------------------------------------------------------------
// US3: Dictionary index list commands
// ---------------------------------------------------------------------------

#[test]
fn cli_compat_article_list_dictionary_index() {
    let dir = setup_compat_repo();
    let (value, stderr, code) = run_json(&dir, &["article", "list", "--project", "2026-blogs", "--json"]);
    assert_eq!(code, 0, "article list: {stderr}");
    assert_eq!(value["status"], "ok");
    assert_eq!(value["data"]["articles"].as_array().expect("articles array").len(), 3);
}

#[test]
fn cli_compat_source_list_dictionary_index() {
    let dir = setup_compat_repo();
    let (value, stderr, code) = run_json(&dir, &["source", "list", "--project", "2026-ai-sites-build", "--json"]);
    assert_eq!(code, 0, "source list: {stderr}");
    assert_eq!(value["status"], "ok");
    let sources = value["data"]["sources"].as_array().expect("sources array");
    assert!(sources.iter().any(|source| source["path"] == "docs/api-spec.md"));
}

#[test]
fn cli_compat_source_list_reads_files_dictionary() {
    let dir = setup_compat_repo();
    let (value, stderr, code) = run_json(&dir, &["source", "list", "--project", "2026-blogs", "--json"]);
    assert_eq!(code, 0, "source list files dictionary: {stderr}");
    assert_eq!(value["status"], "ok");
    let sources = value["data"]["sources"].as_array().expect("sources array");
    assert!(sources.iter().any(|source| source["path"] == "docs/getting-started-with-rust.md"));
    assert!(sources.iter().any(|source| source["path"] == "docs/advanced-patterns.md"));
}

#[test]
fn cli_compat_asset_list_dictionary_index() {
    let dir = setup_compat_repo();
    let (value, stderr, code) = run_json(&dir, &["asset", "list", "--project", "2026-03-hid-prd", "--json"]);
    assert_eq!(code, 0, "asset list: {stderr}");
    assert_eq!(value["status"], "ok");
    assert!(value["data"]["assets"].as_array().expect("assets array").iter().any(|asset| asset["name"] == "logo.png"));
}

// ---------------------------------------------------------------------------
// Lint compatibility
// ---------------------------------------------------------------------------

#[test]
fn cli_compat_project_lint_accepts_local_and_yuque_cc() {
    let dir = setup_compat_repo();
    let (value, _stderr, _code) = run_json(&dir, &["project", "lint", "--json"]);
    assert_eq!(value["status"], "ok", "lint JSON envelope should be ok");
}
