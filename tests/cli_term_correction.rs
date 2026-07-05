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

fn setup_legacy_repo() -> common::TempDir {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    fs::create_dir_all(project.join("docs")).unwrap();
    let index_yaml = r#"schema_version: '1'
terms:
  - term: LegacyProject
    corrections:
      - original: legacy
        correct: LegacyProject
        match: substring
"#;
    common::write_index(&repo, "alpha", index_yaml);
    let global_yaml = r#"schema_version: '1'
terms:
  - term: LegacyGlobal
    corrections:
      - original: legacyg
        correct: LegacyGlobal
        match: substring
"#;
    fs::write(repo.path().join("minds-terms.yaml"), global_yaml).unwrap();
    repo
}

// ── Correction add defaults ──────────────────────────────────────────────

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

// ── Correction add explicit attributes ────────────────────────────────────

#[test]
fn correction_add_accepts_substring_with_safe_default_boundary() {
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

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let index_after = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert!(index_after.contains("match: substring"), "index: {index_after}");
    let show = mf(&repo).args(["term", "correction", "show", "RAG", "ragg", "--project", "alpha"]).output().unwrap();
    let stdout = String::from_utf8(show.stdout).unwrap();
    assert!(stdout.contains("boundary: standalone"), "stdout: {stdout}");
}

// ── T018: Duplicate add is idempotent and reports "already exists, skipped" ─

#[test]
fn correction_add_duplicate_idempotent() {
    let (repo, _project) = setup_repo();

    // First add
    let output1 =
        mf(&repo).args(["term", "correction", "add", "RAG", "dup", "RAG", "--project", "alpha"]).output().unwrap();
    assert!(output1.status.success());
    let stdout1 = String::from_utf8(output1.stdout).unwrap();
    assert!(stdout1.contains("added correction") || stdout1.contains("added"), "first add should say added: {stdout1}");

    // Snapshot storage after first add
    let index_before = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();

    // Second add — idempotent, must report "already exists, skipped"
    let output2 =
        mf(&repo).args(["term", "correction", "add", "RAG", "dup", "RAG", "--project", "alpha"]).output().unwrap();
    assert!(output2.status.success());
    let stdout2 = String::from_utf8(output2.stdout).unwrap();
    assert!(
        stdout2.contains("already exists") && stdout2.contains("skipped"),
        "second add should say already exists, skipped: {stdout2}"
    );

    // Storage must be byte-identical
    let index_after = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert_eq!(index_before, index_after, "second add must not rewrite storage");
}

// ── T019: JSON output has created: true / created: false ──────────────────

#[test]
fn correction_add_json_created_flag() {
    let (repo, _project) = setup_repo();

    // First add — created: true
    let output1 = mf(&repo)
        .args(["--json", "term", "correction", "add", "RAG", "dup2", "RAG", "--project", "alpha"])
        .output()
        .unwrap();
    assert!(output1.status.success());
    let stdout1 = String::from_utf8(output1.stdout).unwrap();
    let v1: serde_json::Value = serde_json::from_str(&stdout1).expect("valid JSON");
    assert_eq!(v1["status"], "ok", "JSON: {stdout1}");
    assert_eq!(v1["data"]["created"], true, "first add should have created: true: {stdout1}");

    // Repeat add — created: false
    let output2 = mf(&repo)
        .args(["--json", "term", "correction", "add", "RAG", "dup2", "RAG", "--project", "alpha"])
        .output()
        .unwrap();
    assert!(output2.status.success());
    let stdout2 = String::from_utf8(output2.stdout).unwrap();
    let v2: serde_json::Value = serde_json::from_str(&stdout2).expect("valid JSON");
    assert_eq!(v2["status"], "ok", "JSON: {stdout2}");
    assert_eq!(v2["data"]["created"], false, "repeat add should have created: false: {stdout2}");
}

// ── T020: Global scope parity for created/no-op ───────────────────────────

#[test]
fn correction_add_global_created_flag() {
    let (repo, _project) = setup_repo();

    // First add to global — created: true
    let output1 =
        mf(&repo).args(["--json", "term", "correction", "add", "GlobalX", "gx2", "GlobalX"]).output().unwrap();
    assert!(output1.status.success());
    let stdout1 = String::from_utf8(output1.stdout).unwrap();
    let v1: serde_json::Value = serde_json::from_str(&stdout1).expect("valid JSON");
    assert_eq!(v1["data"]["created"], true, "first global add: {stdout1}");
    let text1 = v1["data"]["scope"].as_str().unwrap_or("");
    assert_eq!(text1, "global");

    // Repeat add — created: false
    let output2 =
        mf(&repo).args(["--json", "term", "correction", "add", "GlobalX", "gx2", "GlobalX"]).output().unwrap();
    assert!(output2.status.success());
    let stdout2 = String::from_utf8(output2.stdout).unwrap();
    let v2: serde_json::Value = serde_json::from_str(&stdout2).expect("valid JSON");
    assert_eq!(v2["data"]["created"], false, "repeat global add: {stdout2}");
}

// ── Correction list and show ──────────────────────────────────────────────

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

#[test]
fn correction_show_and_list_tolerate_legacy_substring() {
    let repo = setup_legacy_repo();

    let show = mf(&repo)
        .args(["term", "correction", "show", "LegacyProject", "legacy", "--project", "alpha"])
        .output()
        .unwrap();
    assert!(show.status.success(), "stderr: {}", String::from_utf8_lossy(&show.stderr));
    let show_stdout = String::from_utf8(show.stdout).unwrap();
    assert!(show_stdout.contains("match: substring"), "stdout: {show_stdout}");

    let list = mf(&repo).args(["term", "correction", "list", "LegacyGlobal"]).output().unwrap();
    assert!(list.status.success(), "stderr: {}", String::from_utf8_lossy(&list.stderr));
    let list_stdout = String::from_utf8(list.stdout).unwrap();
    assert!(list_stdout.contains("match=substring"), "stdout: {list_stdout}");
}

// ── Correction update ─────────────────────────────────────────────────────

#[test]
fn correction_update_accepts_substring_match() {
    let (repo, _project) = setup_repo();

    let output = mf(&repo)
        .args(["term", "correction", "update", "RAG", "rag", "--match", "substring", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let index_after = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert!(index_after.contains("match: substring"), "index: {index_after}");
}

#[test]
fn correction_update_repairs_legacy_substring_in_project_and_global_scope() {
    let repo = setup_legacy_repo();

    let project_fix = mf(&repo)
        .args(["term", "correction", "update", "LegacyProject", "legacy", "--match", "word", "--project", "alpha"])
        .output()
        .unwrap();
    assert!(project_fix.status.success(), "stderr: {}", String::from_utf8_lossy(&project_fix.stderr));
    let index_after = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert!(index_after.contains("original: legacy"), "index: {index_after}");
    assert!(!index_after.contains("match: substring"), "index: {index_after}");

    let global_fix = mf(&repo)
        .args(["term", "correction", "update", "LegacyGlobal", "legacyg", "--match", "pinyin"])
        .output()
        .unwrap();
    assert!(global_fix.status.success(), "stderr: {}", String::from_utf8_lossy(&global_fix.stderr));
    let global_after = fs::read_to_string(repo.path().join("minds-terms.yaml")).unwrap();
    assert!(global_after.contains("match: pinyin"), "global: {global_after}");
    assert!(!global_after.contains("match: substring"), "global: {global_after}");
}

// ── Correction remove ─────────────────────────────────────────────────────

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

#[test]
fn correction_remove_repairs_legacy_substring_and_dry_run_is_byte_identical() {
    let repo = setup_legacy_repo();
    let project_index = repo.path().join("alpha/mind-index.yaml");
    let global_terms = repo.path().join("minds-terms.yaml");

    let project_before = fs::read_to_string(&project_index).unwrap();
    let dry_run = mf(&repo)
        .args(["term", "correction", "remove", "LegacyProject", "legacy", "--dry-run", "--project", "alpha"])
        .output()
        .unwrap();
    assert!(dry_run.status.success(), "stderr: {}", String::from_utf8_lossy(&dry_run.stderr));
    let project_after_dry_run = fs::read_to_string(&project_index).unwrap();
    assert_eq!(project_before, project_after_dry_run, "dry-run remove must not rewrite legacy project YAML");

    let project_remove = mf(&repo)
        .args(["term", "correction", "remove", "LegacyProject", "legacy", "--project", "alpha"])
        .output()
        .unwrap();
    assert!(project_remove.status.success(), "stderr: {}", String::from_utf8_lossy(&project_remove.stderr));
    let project_after = fs::read_to_string(&project_index).unwrap();
    assert!(!project_after.contains("legacy"), "project correction should be removed: {project_after}");

    let global_remove = mf(&repo).args(["term", "correction", "remove", "LegacyGlobal", "legacyg"]).output().unwrap();
    assert!(global_remove.status.success(), "stderr: {}", String::from_utf8_lossy(&global_remove.stderr));
    let global_after = fs::read_to_string(&global_terms).unwrap();
    assert!(!global_after.contains("legacyg"), "global correction should be removed: {global_after}");
}

// ── Error tests ───────────────────────────────────────────────────────────

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

// ── Global scope parity ───────────────────────────────────────────────────

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
