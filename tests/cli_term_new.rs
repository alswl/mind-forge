use assert_cmd::Command;
use std::fs;

mod common;

fn setup() -> (common::TempDir, std::path::PathBuf) {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    std::fs::create_dir_all(project.join("docs")).unwrap();
    common::write_index(&repo, "alpha", "schema_version: '1'\n");
    (repo, project)
}

// ---------------------------------------------------------------------------
// 1. new_term_happy_full_args — 全参数 happy path
// ---------------------------------------------------------------------------

#[test]
fn new_term_happy_full_args() {
    let (repo, _project) = setup();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "term",
            "new",
            "Mind Repo",
            "--definition",
            "项目仓库根",
            "--alias",
            "mr",
            "--alias",
            "mindrepo",
            "--tag",
            "infra",
            "--tag",
            "product",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Mind Repo"), "stdout: {stdout}");

    // Verify index was written
    let index = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert!(index.contains("Mind Repo"));
    assert!(index.contains("项目仓库根"));
    assert!(index.contains("mr"));
}

// ---------------------------------------------------------------------------
// 2. new_term_alias_tag_dedup — 多次重复去重
// ---------------------------------------------------------------------------

#[test]
fn new_term_alias_tag_dedup() {
    let (repo, _project) = setup();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "term",
            "new",
            "CLI",
            "--alias",
            "cli",
            "--alias",
            "cli",
            "--tag",
            "tool",
            "--tag",
            "tool",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    // 1 alias, 1 tag
    assert!(stdout.contains("1 alias"), "stdout: {stdout}");
    assert!(stdout.contains("1 tag"), "stdout: {stdout}");

    // Verify single entry in index
    let index = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert_eq!(index.matches("- cli").count(), 1, "no dedup: {index}");
}

// ---------------------------------------------------------------------------
// 3. new_term_no_definition — definition=null
// ---------------------------------------------------------------------------

#[test]
fn new_term_no_definition() {
    let (repo, _project) = setup();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "term",
            "new",
            "Test",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());

    let index = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert!(
        index.contains("definition: null") || index.contains("definition: ~"),
        "index: {index}"
    );
}

// ---------------------------------------------------------------------------
// 4. new_term_duplicate_rejected — 同主名重复 → usage exit
// ---------------------------------------------------------------------------

#[test]
fn new_term_duplicate_rejected() {
    let (repo, _project) = setup();
    // First one should succeed
    Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "term",
            "new",
            "Duplicate",
            "--project",
            "alpha",
        ])
        .assert()
        .code(0);

    // Second one should fail
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "term",
            "new",
            "Duplicate",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("already exists"), "stderr: {stderr}");
}

// ---------------------------------------------------------------------------
// 5. new_term_case_sensitive — case differs → distinct terms
// ---------------------------------------------------------------------------

#[test]
fn new_term_case_sensitive() {
    let (repo, _project) = setup();
    Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "term",
            "new",
            "mind repo",
            "--project",
            "alpha",
        ])
        .assert()
        .code(0);

    Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "term",
            "new",
            "Mind Repo",
            "--project",
            "alpha",
        ])
        .assert()
        .code(0);

    let index = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert_eq!(index.matches("term:").count(), 2, "should have 2 terms: {index}");
}

// ---------------------------------------------------------------------------
// 6. new_term_empty_name_rejected — 空字符串 → usage
// ---------------------------------------------------------------------------

#[test]
fn new_term_empty_name_rejected() {
    let (repo, _project) = setup();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "new", "", "--project", "alpha"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("empty"), "stderr: {stderr}");
}

// ---------------------------------------------------------------------------
// 7. new_term_outside_mind_repo — cwd 不在 Repo 内
// ---------------------------------------------------------------------------

#[test]
fn new_term_outside_mind_repo() {
    let output = Command::cargo_bin("mf").unwrap().args(["term", "new", "Test"]).output().unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not in a mind repo"), "stderr: {stderr}");
}

// ---------------------------------------------------------------------------
// 8. new_term_without_project_context — repo root, no project
// ---------------------------------------------------------------------------

#[test]
fn new_term_without_project_context() {
    let repo = common::setup_repo();
    // repo has no projects, so running from root with --root but no --project
    // and cwd not inside a project → should fail
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "new", "Test"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("could not detect"), "stderr: {stderr}");
}

// ---------------------------------------------------------------------------
// 9. new_term_json_output_shape — --format json envelope
// ---------------------------------------------------------------------------

#[test]
fn new_term_json_output_shape() {
    let (repo, _project) = setup();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "--format",
            "json",
            "term",
            "new",
            "Mind Repo",
            "--definition",
            "desc",
            "--alias",
            "mr",
            "--tag",
            "infra",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["status"], "ok");

    let data = &parsed["data"];
    assert_eq!(data["term"], "Mind Repo");
    assert_eq!(data["definition"], "desc");
    assert_eq!(data["aliases"].as_array().unwrap().len(), 1);
    assert_eq!(data["tags"].as_array().unwrap().len(), 1);
    assert_eq!(data["corrections"].as_array().unwrap().len(), 0);
}
