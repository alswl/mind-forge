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
        .args(["--root", repo.path().to_str().unwrap(), "term", "new", "Test", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success());

    let index = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert!(index.contains("definition: null") || index.contains("definition: ~"), "index: {index}");
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
        .args(["--root", repo.path().to_str().unwrap(), "term", "new", "Duplicate", "--project", "alpha"])
        .assert()
        .code(0);

    // Second one should fail
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "new", "Duplicate", "--project", "alpha"])
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
        .args(["--root", repo.path().to_str().unwrap(), "term", "new", "mind repo", "--project", "alpha"])
        .assert()
        .code(0);

    Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "new", "Mind Repo", "--project", "alpha"])
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
// 8. new_term_without_project_context — repo root, no project → global terms
// ---------------------------------------------------------------------------

#[test]
fn new_term_without_project_context() {
    let repo = common::setup_repo();
    // No --project and not inside a project dir → should succeed with global terms
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "new", "Test"])
        .output()
        .unwrap();

    assert!(output.status.success(), "should succeed with global terms");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Test"), "stdout: {stdout}");

    // Verify global terms file was written
    let global_terms = fs::read_to_string(repo.path().join("minds-terms.yaml")).unwrap();
    assert!(global_terms.contains("Test"), "global terms should contain Test: {global_terms}");
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

// ── US2: Repository-format new ─────────────────────────────────────────────

// T024
#[test]
fn new_appends_repo_format() {
    let repo = common::setup_repo();
    write_global_terms(&repo, &repo_format_fixture("simple.yaml"));

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "term",
            "new",
            "巡海",
            "--misrecognition",
            "寻海",
            "--misrecognition",
            "迅海",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "exit 0: {:?}", output.status.code());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("巡海"), "stdout should name new term: {stdout}");
    assert!(stdout.contains("2 misrecognitions"), "should show misrecognition count: {stdout}");

    let content = fs::read_to_string(repo.path().join("minds-terms.yaml")).unwrap();
    assert!(content.contains("巡海:"), "should contain new key: {content}");
    assert!(content.contains("    - 寻海"), "should contain first misrecognition: {content}");
    assert!(content.contains("    - 迅海"), "should contain second misrecognition: {content}");
    assert!(content.contains("cafed:"), "original cafed block must still be present: {content}");
    assert!(content.contains("凯飞迪"), "original misrecognition must be present: {content}");
}

// T025
#[test]
fn new_empty_misrecognitions() {
    let repo = common::setup_repo();
    write_global_terms(&repo, &repo_format_fixture("simple.yaml"));

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "new", "foo"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let content = fs::read_to_string(repo.path().join("minds-terms.yaml")).unwrap();
    assert!(content.contains("foo:\n  misrecognitions: []"), "should get empty misrecognitions list: {content}");
}

// T026
#[test]
fn new_duplicate_rejected_repo_format() {
    let repo = common::setup_repo();
    let fixture = repo_format_fixture("simple.yaml");
    write_global_terms(&repo, &fixture);

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "new", "cafed", "--misrecognition", "foo"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2), "exit 2 for duplicate");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("already exists"), "stderr: {stderr}");

    let after = fs::read_to_string(repo.path().join("minds-terms.yaml")).unwrap();
    assert_eq!(fixture, after, "file must be unchanged after rejected command");
}

// T027
#[test]
fn new_rejects_definition_on_repo_format() {
    let repo = common::setup_repo();
    let fixture = repo_format_fixture("simple.yaml");
    write_global_terms(&repo, &fixture);

    let cases = [
        (vec!["--definition", "foo"], "--definition"),
        (vec!["--alias", "a"], "--alias"),
        (vec!["--tag", "t"], "--tag"),
    ];

    for (args, field_name) in &cases {
        let mut cmd_args = vec!["--root", repo.path().to_str().unwrap(), "term", "new", "x"];
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

// T028
#[test]
fn new_creates_repo_format_when_missing() {
    let repo = common::setup_repo();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "new", "foo", "--misrecognition", "bar"])
        .output()
        .unwrap();

    assert!(output.status.success(), "exit 0: {:?}", output.status.code());
    let content = fs::read_to_string(repo.path().join("minds-terms.yaml")).unwrap();
    assert!(!content.contains("schema_version"), "must not contain schema_version: {content}");
    assert!(!content.contains("schema:"), "must not contain schema alias: {content}");
    assert!(content.contains("foo:\n  misrecognitions:"), "must be repo format: {content}");
    assert!(content.contains("    - bar"), "must contain misrecognition: {content}");
}

// Regression: `--definition=""` (empty/whitespace) on a repo-format file
// must be rejected, not silently slipped through.
#[test]
fn new_empty_definition_rejected_on_repo_format() {
    let repo = common::setup_repo();
    let fixture = repo_format_fixture("simple.yaml");
    write_global_terms(&repo, &fixture);

    for value in &["", "   "] {
        let output = Command::cargo_bin("mf")
            .unwrap()
            .args(["--root", repo.path().to_str().unwrap(), "term", "new", "x", "--definition", value])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(2), "exit 2 for definition={:?}", value);
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("--definition"), "stderr should name --definition: {stderr}");
        assert!(stderr.contains("repository-format"), "stderr should mention repository-format: {stderr}");
    }

    let after = fs::read_to_string(repo.path().join("minds-terms.yaml")).unwrap();
    assert_eq!(fixture, after, "file unchanged after rejected empty-definition commands");
}

// Regression: repeated `--misrecognition` args dedupe (matches the
// existing dedup contract for --alias and --tag).
#[test]
fn new_dedupes_misrecognitions_repo_format() {
    let repo = common::setup_repo();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "term",
            "new",
            "foo",
            "--misrecognition",
            "bar",
            "--misrecognition",
            "bar",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "exit 0: {:?}", output.status.code());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("1 misrecognition"), "should report 1 misrecognition after dedup: {stdout}");

    let content = fs::read_to_string(repo.path().join("minds-terms.yaml")).unwrap();
    assert_eq!(content.matches("- bar").count(), 1, "bar should appear once: {content}");
}

// Regression: prefix collision — `cafe` and `cafed` must be distinct
// when locating term lines during repo-format learn surgical edits.
// The file lists `cafed:` BEFORE `cafe:` so a naive `starts_with("cafe:")`
// would match the `cafed:` line first and insert `kafe` into the wrong
// block.
#[test]
fn learn_distinguishes_prefix_term_names_repo_format() {
    let repo = common::setup_repo();
    let content = "cafed:\n  misrecognitions:\n    - 凯飞迪\ncafe:\n  misrecognitions:\n    - cofee\n";
    write_global_terms(&repo, content);

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "learn", "--term", "cafe", "--alias", "kafe"])
        .output()
        .unwrap();
    assert!(output.status.success(), "exit 0: {:?}", output.status.code());

    let after = fs::read_to_string(repo.path().join("minds-terms.yaml")).unwrap();
    let cafed_block_end = after.find("cafe:\n").expect("cafe block follows cafed block");
    let cafed_block = &after[..cafed_block_end];
    assert!(!cafed_block.contains("kafe"), "kafe must NOT land in the cafed block: {cafed_block}");
    let cafe_block = &after[cafed_block_end..];
    assert!(cafe_block.contains("kafe"), "kafe must land in the cafe block: {cafe_block}");
}

// T030
#[test]
fn new_schema_version_misrecognition_flag() {
    let repo = common::setup_repo();
    // Write a schema-version global terms file
    std::fs::write(repo.path().join("minds-terms.yaml"), "schema_version: '1'\nterms: []\n").unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "new", "foo", "--misrecognition", "bar"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let content = fs::read_to_string(repo.path().join("minds-terms.yaml")).unwrap();
    assert!(content.contains("schema_version: '1'"), "must remain schema-version: {content}");
    assert!(content.contains("term: foo"), "should contain new term: {content}");
    assert!(content.contains("original: bar"), "should contain correction: {content}");
    assert!(content.contains("correct: foo"), "should contain correction target: {content}");
}
