use assert_cmd::Command;
use tempfile::TempDir;

mod common;

fn seed_terms(repo: &TempDir, project_name: &str) {
    common::write_term_index(
        repo,
        project_name,
        "term: Mind Repo\n    definition: A repository of minds\n    corrections:\n      - original: mindrepo\n        correct: Mind Repo",
    );
}

fn setup_with_terms() -> TempDir {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    seed_terms(&repo, "alpha");
    repo
}

fn seed_two_terms(repo: &TempDir, project_name: &str) {
    let yaml = r#"schema_version: '1'
terms:
  - term: Alpha
    definition: First term
    aliases: []
    tags: []
    corrections: []
  - term: Beta
    definition: Second term
    aliases: []
    tags: []
    corrections: []
"#;
    common::write_index(repo, project_name, yaml);
}

fn setup_with_two_terms() -> TempDir {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    seed_two_terms(&repo, "alpha");
    repo
}

// ---------------------------------------------------------------------------
// 1. rename_term_success — term renamed in index
// ---------------------------------------------------------------------------

#[test]
fn rename_term_success() {
    let repo = setup_with_terms();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "term",
            "rename",
            "Mind Repo",
            "Knowledge Base",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let index_content = std::fs::read_to_string(repo.path().join("alpha").join("mind-index.yaml")).unwrap();
    // The correction still references "Mind Repo" as correct, so we check the term field specifically
    let v: serde_yaml::Value = serde_yaml::from_str(&index_content).unwrap();
    let term_names: Vec<&str> = v["terms"].as_sequence().unwrap().iter().filter_map(|t| t["term"].as_str()).collect();
    assert!(!term_names.contains(&"Mind Repo"), "old term name should be gone");
    assert!(term_names.contains(&"Knowledge Base"), "new term name should be present");
}

// ---------------------------------------------------------------------------
// 2. rename_term_keep_alias — --keep-alias preserves old name as alias
// ---------------------------------------------------------------------------

#[test]
fn rename_term_keep_alias() {
    let repo = setup_with_terms();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "term",
            "rename",
            "Mind Repo",
            "Knowledge Base",
            "--keep-alias",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let index_content = std::fs::read_to_string(repo.path().join("alpha").join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("Knowledge Base"), "new name should be present");
    assert!(index_content.contains("Mind Repo"), "old name should be kept as alias");
}

// ---------------------------------------------------------------------------
// 3. rename_term_duplicate_refusal — renaming to existing term fails without --force
// ---------------------------------------------------------------------------

#[test]
fn rename_term_duplicate_refusal() {
    let repo = setup_with_two_terms();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "rename", "Alpha", "Beta", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("already exists"), "stderr: {stderr}");
}

// ---------------------------------------------------------------------------
// 4. rename_term_dry_run — dry run does not mutate index
// ---------------------------------------------------------------------------

#[test]
fn rename_term_dry_run() {
    let repo = setup_with_terms();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "term",
            "rename",
            "Mind Repo",
            "Knowledge Base",
            "--dry-run",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let index_content = std::fs::read_to_string(repo.path().join("alpha").join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("Mind Repo"), "old name should still exist after dry run");
    assert!(!index_content.contains("Knowledge Base"), "new name should not appear after dry run");
}

// ---------------------------------------------------------------------------
// 5. rename_term_json_envelope — JSON output with full lifecycle envelope
// ---------------------------------------------------------------------------

#[test]
fn rename_term_json_envelope() {
    let repo = setup_with_terms();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "term",
            "rename",
            "Mind Repo",
            "Knowledge Base",
            "--project",
            "alpha",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    assert_eq!(v["data"]["verb"], "rename");
    assert_eq!(v["data"]["kind"], "term");
    assert_eq!(v["data"]["before"]["name"], "Mind Repo");
    assert_eq!(v["data"]["after"]["name"], "Knowledge Base");
    assert_eq!(v["data"]["force"], false);
    assert_eq!(v["data"]["dry_run"], false);
}

// ---------------------------------------------------------------------------
// 6. rename_term_not_found — unknown term → usage error
// ---------------------------------------------------------------------------

#[test]
fn rename_term_not_found() {
    let repo = setup_with_terms();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "term",
            "rename",
            "Nonexistent",
            "Whatever",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not found"), "stderr: {stderr}");
}
