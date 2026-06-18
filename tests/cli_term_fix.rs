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
    tags:
      - infra
    corrections:
      - original: mindrepo
        correct: Mind Repo
  - term: Other
    aliases: []
    tags: []
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
// 1. replace definition
// ---------------------------------------------------------------------------

#[test]
fn fix_term_replace_definition() {
    let (repo, _project) = setup_with_term();
    let output = mf(&repo)
        .args(["term", "update", "Mind Repo", "--definition", "new definition", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("✓ updated term:"), "stdout: {stdout}");

    // Verify index
    let index = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert!(index.contains("new definition"), "index: {index}");
}

// ---------------------------------------------------------------------------
// 2. append alias and tag
// ---------------------------------------------------------------------------

#[test]
fn fix_term_append_alias_and_tag() {
    let (repo, _project) = setup_with_term();
    let output = mf(&repo)
        .args(["term", "update", "Mind Repo", "--alias", "new-alias", "--tag", "new-tag", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("✓ updated term: Mind Repo (2 fields)"), "stdout: {stdout}");

    let index = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert!(index.contains("new-alias"), "alias added: {index}");
    assert!(index.contains("new-tag"), "tag added: {index}");
}

// ---------------------------------------------------------------------------
// 3. combined changes atomic
// ---------------------------------------------------------------------------

#[test]
fn fix_term_combined_changes_atomic() {
    let (repo, _project) = setup_with_term();
    let output = mf(&repo)
        .args([
            "term",
            "update",
            "Other",
            "--definition",
            "combined",
            "--alias",
            "new-a",
            "--tag",
            "new-t",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());

    let index = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert!(index.contains("combined"));
    assert!(index.contains("new-a"));
    assert!(index.contains("new-t"));
}

// ---------------------------------------------------------------------------
// 4. existing alias silently deduped
// ---------------------------------------------------------------------------

#[test]
fn fix_term_silently_ignores_existing_alias() {
    let (repo, _project) = setup_with_term();
    // mr already exists as alias
    let output =
        mf(&repo).args(["term", "update", "Mind Repo", "--alias", "mr", "--project", "alpha"]).output().unwrap();

    assert!(output.status.success());
    // The CLI reports argument count, but the service deduplicates.
    // Verify index still has only 1 alias entry.
    let index = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert_eq!(index.matches("- mr").count(), 1, "no duplicate alias: {index}");
}

// ---------------------------------------------------------------------------
// 5. term not found → usage
// ---------------------------------------------------------------------------

#[test]
fn fix_term_not_found_rejected() {
    let (repo, _project) = setup_with_term();
    let output =
        mf(&repo).args(["term", "update", "NonExistent", "--definition", "x", "--project", "alpha"]).output().unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not found"), "stderr: {stderr}");
}

// ---------------------------------------------------------------------------
// 6. no change flags → usage
// ---------------------------------------------------------------------------

#[test]
fn fix_term_no_change_flags_rejected() {
    let (repo, _project) = setup_with_term();
    let output = mf(&repo).args(["term", "update", "Mind Repo", "--project", "alpha"]).output().unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("at least one"), "stderr: {stderr}");
}

// ---------------------------------------------------------------------------
// 7. corrections unchanged
// ---------------------------------------------------------------------------

#[test]
fn fix_term_does_not_touch_corrections() {
    let (repo, _project) = setup_with_term();
    // Mind Repo has 1 correction. After fix, should still have 1.
    mf(&repo).args(["term", "update", "Mind Repo", "--definition", "updated", "--project", "alpha"]).assert().code(0);

    let index = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    // Count corrections (original: mindrepo should be there once)
    assert!(index.contains("original: mindrepo"));
}

// ---------------------------------------------------------------------------
// 8. JSON shape
// ---------------------------------------------------------------------------

#[test]
fn fix_term_json_shape() {
    let (repo, _project) = setup_with_term();
    let output = mf(&repo)
        .args(["--format", "json", "term", "update", "Mind Repo", "--definition", "json-test", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["data"]["kind"], "term");
    assert_eq!(parsed["data"]["identity"], "Mind Repo");
    assert!(parsed["data"]["details"]["changes"].is_object());
    assert!(!parsed["data"]["dry_run"].as_bool().unwrap());
}

// T052
#[test]
fn fix_schema_version_unchanged() {
    let repo = common::setup_repo();
    // Write a schema-version global terms file
    let schema_yaml = "schema_version: '1'\nterms:\n  - term: cafed\n    definition: null\n    aliases: []\n    tags: []\n    corrections: []\n";
    std::fs::write(repo.path().join("minds-terms.yaml"), schema_yaml).unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "update", "cafed", "--definition", "the cafed thing"])
        .output()
        .unwrap();

    assert!(output.status.success(), "exit 0: {:?}", output.status.code());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("✓ updated term:"), "stdout: {stdout}");

    let content = fs::read_to_string(repo.path().join("minds-terms.yaml")).unwrap();
    assert!(content.contains("the cafed thing"), "definition updated: {content}");
    assert!(content.contains("schema_version"), "must remain schema-version: {content}");
}
