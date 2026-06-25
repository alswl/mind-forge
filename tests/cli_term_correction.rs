use assert_cmd::Command;
use std::fs;

mod common;

fn setup_repo() -> (common::TempDir, std::path::PathBuf) {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    fs::create_dir_all(project.join("docs")).unwrap();
    let index_yaml = r#"schema_version: '1'
terms:
  - term: RAG
    definition: Retrieval augmented generation
    aliases: []
    tags: []
    corrections:
      - original: rag
        correct: RAG
        match: word
        fix: required
"#;
    common::write_index(&repo, "alpha", index_yaml);

    // Global term for parity tests
    let global_yaml = r#"schema_version: '1'
terms:
  - term: GlobalX
    definition: A global term
    aliases: []
    tags: []
    corrections:
      - original: gx
        correct: GlobalX
        match: word
        fix: required
"#;
    std::fs::write(repo.path().join("minds-terms.yaml"), global_yaml).unwrap();

    (repo, project)
}

fn mf(repo: &common::TempDir) -> Command {
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.args(["--root", repo.path().to_str().unwrap()]);
    cmd
}

// ── T037: Correction add tests ───────────────────────────────────────────

#[test]
fn correction_add_defaults() {
    let (repo, _project) = setup_repo();

    let output =
        mf(&repo).args(["term", "correction", "add", "RAG", "ragg", "RAG", "--project", "alpha"]).output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("RAG"), "stdout: {stdout}");
    assert!(stdout.contains("ragg"), "stdout: {stdout}");

    // Verify storage
    let index = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert!(index.contains("ragg"), "index should contain new correction: {index}");
}

#[test]
fn correction_add_explicit_attributes() {
    let (repo, _project) = setup_repo();

    let output = mf(&repo)
        .args([
            "term",
            "correction",
            "add",
            "RAG",
            "ragg",
            "RAG",
            "--match",
            "substring",
            "--fix",
            "suggested",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let index = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert!(index.contains("match: substring"), "index: {index}");
    assert!(index.contains("fix: suggested"), "index: {index}");
}

#[test]
fn correction_add_duplicate_idempotent() {
    let (repo, _project) = setup_repo();

    // First add
    mf(&repo).args(["term", "correction", "add", "RAG", "dup", "RAG", "--project", "alpha"]).assert().success();

    // Second add should succeed (idempotent)
    let output =
        mf(&repo).args(["term", "correction", "add", "RAG", "dup", "RAG", "--project", "alpha"]).output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    // Should mention already exists or be idempotent
    assert!(stdout.contains("RAG") || stdout.contains("dup"), "stdout: {stdout}");
}

// ── T038: Correction list and show tests ─────────────────────────────────

#[test]
fn correction_list_text() {
    let (repo, _project) = setup_repo();

    let output = mf(&repo).args(["term", "correction", "list", "RAG", "--project", "alpha"]).output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("rag"), "stdout: {stdout}");
    assert!(stdout.contains("RAG"), "stdout: {stdout}");
}

#[test]
fn correction_list_json() {
    let (repo, _project) = setup_repo();

    let output =
        mf(&repo).args(["--json", "term", "correction", "list", "RAG", "--project", "alpha"]).output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(v["status"], "ok", "JSON: {stdout}");
}

#[test]
fn correction_show() {
    let (repo, _project) = setup_repo();

    let output = mf(&repo).args(["term", "correction", "show", "RAG", "rag", "--project", "alpha"]).output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("rag"), "stdout: {stdout}");
}

// ── T039: Correction update tests ────────────────────────────────────────

#[test]
fn correction_update_change_match() {
    let (repo, _project) = setup_repo();

    let output = mf(&repo)
        .args(["term", "correction", "update", "RAG", "rag", "--match", "substring", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("rag"), "stdout: {stdout}");

    let index = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert!(index.contains("match: substring"), "index: {index}");
}

// ── T040: Correction remove tests ────────────────────────────────────────

#[test]
fn correction_remove() {
    let (repo, _project) = setup_repo();

    let output = mf(&repo).args(["term", "correction", "remove", "RAG", "rag", "--project", "alpha"]).output().unwrap();

    assert!(output.status.success());
    let index = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert!(!index.contains("original: rag"), "correction should be removed: {index}");
}

#[test]
fn correction_remove_dry_run() {
    let (repo, _project) = setup_repo();
    let index_before = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();

    let output = mf(&repo)
        .args(["term", "correction", "remove", "RAG", "rag", "--dry-run", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("dry-run") || stdout.contains("[dry-run]"), "stdout: {stdout}");

    let index_after = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert_eq!(index_before, index_after, "dry-run must not write");
}

// ── T041: Error tests ────────────────────────────────────────────────────

#[test]
fn correction_add_missing_parent_term() {
    let (repo, _project) = setup_repo();

    let output =
        mf(&repo).args(["term", "correction", "add", "NoSuchTerm", "x", "Y", "--project", "alpha"]).output().unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not found") || stderr.contains("NoSuchTerm"), "stderr: {stderr}");
}

#[test]
fn correction_update_missing_correction() {
    let (repo, _project) = setup_repo();

    let output = mf(&repo)
        .args(["term", "correction", "update", "RAG", "nonexistent", "--match", "word", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not found") || stderr.contains("nonexistent"), "stderr: {stderr}");
}

// ── T042: Project/global scope parity tests ──────────────────────────────

#[test]
fn correction_add_global() {
    let (repo, _project) = setup_repo();

    let output = mf(&repo).args(["term", "correction", "add", "GlobalX", "gx2", "GlobalX"]).output().unwrap();

    assert!(output.status.success());
    let global = fs::read_to_string(repo.path().join("minds-terms.yaml")).unwrap();
    assert!(global.contains("gx2"), "global terms should contain new correction: {global}");
}

#[test]
fn correction_list_global() {
    let (repo, _project) = setup_repo();

    let output = mf(&repo).args(["term", "correction", "list", "GlobalX"]).output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("gx"), "stdout: {stdout}");
}
