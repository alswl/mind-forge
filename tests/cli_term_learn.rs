use assert_cmd::Command;
use std::fs;

mod common;

fn setup_with_term() -> (common::TempDir, std::path::PathBuf) {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    fs::create_dir_all(project.join("docs")).unwrap();
    let index_yaml = r#"schema_version: '1'
terms:
  - term: Mind Repo
    definition: 项目仓库根
    aliases:
      - mr
      - mindrepo
    tags:
      - infra
    corrections: []
  - term: Other
    aliases:
      - other-alias
    corrections: []
"#;
    common::write_index(&repo, "alpha", index_yaml);
    (repo, project)
}

fn mf(repo: &common::TempDir) -> Command {
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.args(["--root", repo.path().to_str().unwrap()]);
    cmd
}

// ---------------------------------------------------------------------------
// 1. learn basic append — main name hit
// ---------------------------------------------------------------------------

#[test]
fn learn_basic_append() {
    let (repo, _project) = setup_with_term();
    let output = mf(&repo)
        .args(["term", "learn", "--original", "old-mindrepo", "--correct", "Mind Repo", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("learned"), "stdout: {stdout}");

    // Verify index was updated
    let index = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert!(index.contains("old-mindrepo"), "correction added: {index}");
}

// ---------------------------------------------------------------------------
// 2. learn idempotent — same pair again
// ---------------------------------------------------------------------------

#[test]
fn learn_idempotent_when_pair_exists() {
    let (repo, _project) = setup_with_term();
    // First — should succeed
    mf(&repo)
        .args(["term", "learn", "--original", "mr-old", "--correct", "Mind Repo", "--project", "alpha"])
        .assert()
        .code(0);

    // Second — should be idempotent
    let output = mf(&repo)
        .args(["term", "learn", "--original", "mr-old", "--correct", "Mind Repo", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("already exists"), "idempotent: {stdout}");
}

// ---------------------------------------------------------------------------
// 3. correct unknown → usage
// ---------------------------------------------------------------------------

#[test]
fn learn_correct_unknown_rejected() {
    let (repo, _project) = setup_with_term();
    let output = mf(&repo)
        .args(["term", "learn", "--original", "foo", "--correct", "NonExistent", "--project", "alpha"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("no term registers"), "stderr: {stderr}");
}

// ---------------------------------------------------------------------------
// 4. correct ambiguous → usage
// ---------------------------------------------------------------------------

#[test]
fn learn_correct_ambiguous_rejected() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    fs::create_dir_all(project.join("docs")).unwrap();
    // Two terms both claiming the same alias
    let index_yaml = r#"schema_version: '1'
terms:
  - term: Alpha
    aliases:
      - shared
    corrections: []
  - term: Beta
    aliases:
      - shared
    corrections: []
"#;
    common::write_index(&repo, "alpha", index_yaml);

    let output = mf(&repo)
        .args(["term", "learn", "--original", "foo", "--correct", "shared", "--project", "alpha"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("multiple terms claim"), "stderr: {stderr}");
}

// ---------------------------------------------------------------------------
// 5. original equals term or alias → usage
// ---------------------------------------------------------------------------

#[test]
fn learn_original_equals_term_or_alias_rejected() {
    let (repo, _project) = setup_with_term();
    // original equals term name
    let output = mf(&repo)
        .args(["term", "learn", "--original", "Mind Repo", "--correct", "Mind Repo", "--project", "alpha"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("already a recognized form"), "stderr: {stderr}");

    // original equals alias
    let output2 = mf(&repo)
        .args(["term", "learn", "--original", "mr", "--correct", "Mind Repo", "--project", "alpha"])
        .output()
        .unwrap();

    assert_eq!(output2.status.code(), Some(2));
    let stderr2 = String::from_utf8(output2.stderr).unwrap();
    assert!(stderr2.contains("already a recognized form"), "stderr2: {stderr2}");
}

// ---------------------------------------------------------------------------
// 6. empty original or correct → usage
// ---------------------------------------------------------------------------

#[test]
fn learn_empty_original_or_correct_rejected() {
    let (repo, _project) = setup_with_term();
    // Empty original is a clap error? No — it's a flag, so it passes clap.
    // The empty check is in the service.
    let output =
        mf(&repo).args(["term", "learn", "--alias", "", "--term", "Mind Repo", "--project", "alpha"]).output().unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("requires"), "stderr: {stderr}");
}

// ---------------------------------------------------------------------------
// 7. JSON shape
// ---------------------------------------------------------------------------

#[test]
fn learn_json_shape() {
    let (repo, _project) = setup_with_term();
    let output = mf(&repo)
        .args([
            "--format",
            "json",
            "term",
            "learn",
            "--original",
            "old-term",
            "--correct",
            "Mind Repo",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["data"]["term"], "Mind Repo");
    assert!(!parsed["data"]["corrections"].as_array().unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// 8. --project alpha
// ---------------------------------------------------------------------------

#[test]
fn learn_with_project_root_flags() {
    let (repo, _project) = setup_with_term();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "term",
            "learn",
            "--original",
            "learned-term",
            "--correct",
            "Mind Repo",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
}

