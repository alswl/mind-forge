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

fn repo_format_fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/term_repo_format").join(name),
    )
    .unwrap()
}

fn write_global_terms(repo: &common::TempDir, content: &str) {
    std::fs::write(repo.path().join("minds-terms.yaml"), content).unwrap();
}

// ---------------------------------------------------------------------------
// 1. replace definition
// ---------------------------------------------------------------------------

#[test]
fn fix_term_replace_definition() {
    let (repo, _project) = setup_with_term();
    let output = mf(&repo)
        .args(["term", "fix", "Mind Repo", "--definition", "new definition", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("definition changed"), "stdout: {stdout}");

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
        .args(["term", "fix", "Mind Repo", "--alias", "new-alias", "--tag", "new-tag", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("+1 alias"), "stdout: {stdout}");
    assert!(stdout.contains("+1 tag"), "stdout: {stdout}");

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
            "fix",
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
    let output = mf(&repo).args(["term", "fix", "Mind Repo", "--alias", "mr", "--project", "alpha"]).output().unwrap();

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
        mf(&repo).args(["term", "fix", "NonExistent", "--definition", "x", "--project", "alpha"]).output().unwrap();

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
    let output = mf(&repo).args(["term", "fix", "Mind Repo", "--project", "alpha"]).output().unwrap();

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
    mf(&repo).args(["term", "fix", "Mind Repo", "--definition", "updated", "--project", "alpha"]).assert().code(0);

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
        .args(["--format", "json", "term", "fix", "Mind Repo", "--definition", "json-test", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["data"]["term"], "Mind Repo");
    assert_eq!(parsed["data"]["definition"], "json-test");
    assert!(parsed["data"].get("aliases").is_some());
    assert!(parsed["data"].get("tags").is_some());
    assert!(parsed["data"].get("corrections").is_some());
}

// ── US4: Repository-format fix rejection ───────────────────────────────────

// T051
#[test]
fn fix_rejects_definition_on_repo_format() {
    let repo = common::setup_repo();
    let fixture = repo_format_fixture("simple.yaml");
    write_global_terms(&repo, &fixture);

    let cases = [
        (vec!["--definition", "foo"], "--definition"),
        (vec!["--alias", "a"], "--alias"),
        (vec!["--tag", "t"], "--tag"),
    ];

    for (args, field_name) in &cases {
        let mut cmd_args = vec!["--root", repo.path().to_str().unwrap(), "term", "fix", "cafed"];
        cmd_args.extend(args.iter().copied());

        let output = Command::cargo_bin("mf").unwrap().args(&cmd_args).output().unwrap();

        assert_eq!(output.status.code(), Some(2), "exit 2 for {field_name}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(field_name), "stderr should name {field_name}: {stderr}");
        assert!(stderr.contains("repository-format"), "stderr should mention repository-format: {stderr}");
    }

    let after = fs::read_to_string(repo.path().join("minds-terms.yaml")).unwrap();
    assert_eq!(fixture, after, "file unchanged after all rejected commands");
}

#[test]
fn fix_rejects_schema_tagged_repo_format() {
    let repo = common::setup_repo();
    let fixture = "# 会议纪要术语校准表\nschema_version: '1'\n\ncafed:\n  misrecognitions:\n    - 凯飞迪\n";
    write_global_terms(&repo, fixture);

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "fix", "cafed", "--definition", "updated"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2), "exit 2 for repo-format fix");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--definition"), "stderr should name --definition: {stderr}");
    assert!(stderr.contains("repository-format"), "stderr should mention repository-format: {stderr}");

    let after = fs::read_to_string(repo.path().join("minds-terms.yaml")).unwrap();
    assert_eq!(fixture, after, "file unchanged after rejected fix");
}

// Regression: `--definition=""` (empty string) on a repo-format file must
// be rejected, NOT silently accepted and committed via a schema-version
// rewrite that destroys comments. See FR-013 / FR-008 / FR-010.
#[test]
fn fix_empty_definition_rejected_on_repo_format() {
    let repo = common::setup_repo();
    let fixture = repo_format_fixture("simple.yaml");
    write_global_terms(&repo, &fixture);

    for value in &["", "   "] {
        let output = Command::cargo_bin("mf")
            .unwrap()
            .args(["--root", repo.path().to_str().unwrap(), "term", "fix", "cafed", "--definition", value])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(2), "exit 2 for definition={:?}", value);
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("--definition"), "stderr should name --definition: {stderr}");
        assert!(stderr.contains("repository-format"), "stderr should mention repository-format: {stderr}");
    }

    let after = fs::read_to_string(repo.path().join("minds-terms.yaml")).unwrap();
    assert_eq!(fixture, after, "file unchanged after rejected empty-definition commands");
    assert!(!after.contains("schema_version"), "must NOT be rewritten to schema-version: {after}");
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
        .args(["--root", repo.path().to_str().unwrap(), "term", "fix", "cafed", "--definition", "the cafed thing"])
        .output()
        .unwrap();

    assert!(output.status.success(), "exit 0: {:?}", output.status.code());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("definition changed"), "stdout: {stdout}");

    let content = fs::read_to_string(repo.path().join("minds-terms.yaml")).unwrap();
    assert!(content.contains("the cafed thing"), "definition updated: {content}");
    assert!(content.contains("schema_version"), "must remain schema-version: {content}");
}
