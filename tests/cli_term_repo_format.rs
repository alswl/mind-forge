//! Integration tests: repo-format detection, duplicate-key rejection,
//! and read-only byte-equality assertions.
//!
//! Covers contracts/repo-format-detection.md acceptance checks.

use assert_cmd::Command;
use std::fs;

mod common;

fn write_terms_file(repo: &common::TempDir, content: &str) {
    fs::write(repo.path().join("minds-terms.yaml"), content).unwrap();
}

fn fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/term_repo_format").join(name);
    fs::read_to_string(&path).unwrap()
}

// ── Format detection via CLI (list) ────────────────────────────────────────

#[test]
fn list_empty_file_is_repository_zero_terms() {
    let repo = common::setup_repo();
    write_terms_file(&repo, "");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "list"])
        .output()
        .unwrap();

    assert!(output.status.success(), "should exit 0 for empty file");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("No terms found"), "got: {stdout}");
}

#[test]
fn list_null_file_is_repository_zero_terms() {
    let repo = common::setup_repo();
    write_terms_file(&repo, "null\n");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "list"])
        .output()
        .unwrap();

    assert!(output.status.success(), "should exit 0 for null file");
}

#[test]
fn list_repo_format_success() {
    let repo = common::setup_repo();
    write_terms_file(&repo, &fixture("simple.yaml"));

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "list"])
        .output()
        .unwrap();

    assert!(output.status.success(), "should exit 0 for repo-format file");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("cafed"), "should list cafed: {stdout}");
    assert!(stdout.contains("IGO"), "should list IGO: {stdout}");
}

#[test]
fn list_repo_format_json() {
    let repo = common::setup_repo();
    write_terms_file(&repo, &fixture("simple.yaml"));

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "--format", "json", "term", "list"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["status"], "ok");
    let data = parsed["data"].as_array().unwrap();
    assert_eq!(data.len(), 3);
    assert_eq!(data[0]["term"], "IGO");
    assert_eq!(data[0]["definition"], serde_json::Value::Null);
    assert_eq!(data[0]["aliases"].as_array().unwrap().len(), 0);
    assert_eq!(data[0]["tags"].as_array().unwrap().len(), 0);
    let corr = data[0]["corrections"].as_array().unwrap();
    assert_eq!(corr[0]["original"], "Igo");
    assert_eq!(corr[0]["correct"], "IGO");
}

#[test]
fn list_mixed_shape_is_schema_version() {
    let repo = common::setup_repo();
    write_terms_file(&repo, &fixture("mixed.yaml"));

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "list"])
        .output()
        .unwrap();

    // schema_version wins → treats as schema-version. terms: [] so empty.
    assert!(output.status.success());
}

#[test]
fn list_malformed_rejected() {
    let repo = common::setup_repo();
    write_terms_file(&repo, &fixture("malformed.yaml"));

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "list"])
        .output()
        .unwrap();

    assert!(!output.status.success(), "should fail on malformed file");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("unsupported file shape"), "stderr should name unsupported shape: {stderr}");
}

#[test]
fn list_top_level_sequence_rejected() {
    let repo = common::setup_repo();
    write_terms_file(&repo, "- list-at-root\n");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "list"])
        .output()
        .unwrap();

    assert!(!output.status.success(), "should fail on top-level sequence");
}

#[test]
fn list_duplicate_key_rejected() {
    let repo = common::setup_repo();
    write_terms_file(&repo, &fixture("duplicate.yaml"));

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "list"])
        .output()
        .unwrap();

    assert!(!output.status.success(), "should fail on duplicate key");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("duplicate term"), "stderr should mention duplicate term: {stderr}");
}

// T020: read-only commands must not modify the file (SC-005)
#[test]
fn list_does_not_modify_file() {
    let repo = common::setup_repo();
    write_terms_file(&repo, &fixture("simple.yaml"));

    let before = fs::read(repo.path().join("minds-terms.yaml")).unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "list"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let after = fs::read(repo.path().join("minds-terms.yaml")).unwrap();
    assert_eq!(before, after, "bytes must be unchanged after read-only command");
}

#[test]
fn show_does_not_modify_file() {
    let repo = common::setup_repo();
    write_terms_file(&repo, &fixture("simple.yaml"));

    let before = fs::read(repo.path().join("minds-terms.yaml")).unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "show", "cafed"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let after = fs::read(repo.path().join("minds-terms.yaml")).unwrap();
    assert_eq!(before, after, "bytes must be unchanged after show");
}

// T029: new preserves comments snapshot (insta) — SC-002 boundary
#[test]
fn new_preserves_comments_snapshot() {
    let repo = common::setup_repo();
    write_terms_file(&repo, &fixture("simple.yaml"));

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "new", "巡海", "--misrecognition", "寻海"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let content = String::from_utf8(fs::read(repo.path().join("minds-terms.yaml")).unwrap()).unwrap();
    insta::assert_snapshot!(content);
}

// T043: learn preserves comments snapshot (insta) — SC-002 boundary
#[test]
fn learn_preserves_comments_snapshot() {
    let repo = common::setup_repo();
    write_terms_file(&repo, &fixture("simple.yaml"));

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "learn", "--term", "cafed", "--alias", "卡维地"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let content = String::from_utf8(fs::read(repo.path().join("minds-terms.yaml")).unwrap()).unwrap();
    insta::assert_snapshot!(content);
}

// T048: lint parity — verify that terms loaded from repo-format and
// schema-version files with logically-equivalent corrections produce
// identical in-memory projections. The lint scanner consumes
// (original, correct, term) tuples from Term.corrections; this test
// locks down the format-agnostic projection guarantee.
#[test]
fn lint_parity_repo_vs_schema_version() {
    // Repo A: repo-format with corrections
    let repo_a = common::setup_repo();
    let repo_fixture = fixture("simple.yaml");
    fs::write(repo_a.path().join("minds-terms.yaml"), &repo_fixture).unwrap();

    // Repo B: schema-version with equivalent corrections
    let repo_b = common::setup_repo();
    let schema_yaml = r#"schema_version: '1'
terms:
  - term: cafed
    definition: null
    aliases: []
    tags: []
    corrections:
      - original: 凯飞迪
        correct: cafed
      - original: caféd
        correct: cafed
  - term: IGO
    definition: null
    aliases: []
    tags: []
    corrections:
      - original: Igo
        correct: IGO
      - original: iGo
        correct: IGO
  - term: 卿祤
    definition: null
    aliases: []
    tags: []
    corrections:
      - original: 庆宇
        correct: 卿祤
      - original: 清雨
        correct: 卿祤
"#;
    fs::write(repo_b.path().join("minds-terms.yaml"), schema_yaml).unwrap();

    // List both as JSON, compare the data payloads
    let list = |repo: &common::TempDir| -> serde_json::Value {
        let output = Command::cargo_bin("mf")
            .unwrap()
            .args(["--root", repo.path().to_str().unwrap(), "--format", "json", "term", "list"])
            .output()
            .unwrap();
        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout).unwrap();
        serde_json::from_str(&stdout).unwrap()
    };

    let json_a = list(&repo_a);
    let json_b = list(&repo_b);

    // Data arrays should be equal (same terms, same corrections, same order)
    assert_eq!(json_a["data"], json_b["data"], "repo-format and schema-version must project identically");
}
