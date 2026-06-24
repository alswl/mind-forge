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
    assert!(stdout.contains("created term \"CLI\""), "stdout: {stdout}");

    // Verify single entry in index (dedup verified at filesystem level)
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
// 4. new_term_duplicate_is_idempotent — same term twice succeeds (US2: append, not error)
// ---------------------------------------------------------------------------

#[test]
fn new_term_duplicate_is_idempotent() {
    let (repo, _project) = setup();
    // First one should succeed
    Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "new", "Duplicate", "--project", "alpha"])
        .assert()
        .code(0);

    // Second one should also succeed (idempotent, no error)
    Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "new", "Duplicate", "--project", "alpha"])
        .assert()
        .code(0);
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
// 9. new_term_json_output_shape — --output json envelope
// ---------------------------------------------------------------------------

#[test]
fn new_term_json_output_shape() {
    let (repo, _project) = setup();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "--output",
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
    assert_eq!(data["created"], true);
    assert_eq!(data["added_aliases"].as_array().unwrap().len(), 1);
    assert_eq!(data["added_tags"].as_array().unwrap().len(), 1);
    assert!(data["added_misrecognitions"].as_array().unwrap().is_empty());
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

// ═══════════════════════════════════════════════════════════════════════════════
// T024 — canonical alias attachment via term new
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn term_new_attaches_alias_to_existing_term() {
    let repo = common::setup_repo();
    std::fs::write(repo.path().join("minds-terms.yaml"), "schema_version: '1'\nterms: []\n").unwrap();

    // Seed a term so `term new --alias` can attach to it.
    Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "new", "FooBar"])
        .assert()
        .code(0);

    // term add removed — use `term new <NAME> --alias ALIAS` instead
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "new", "FooBar", "--alias", "foobar"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    // Verify the alias actually landed.
    let content = fs::read_to_string(repo.path().join("minds-terms.yaml")).unwrap();
    assert!(content.contains("foobar"), "alias should be attached: {content}");
}

// ═══════════════════════════════════════════════════════════════════════════════
// T035 — Idempotency: second identical invocation is a no-op
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn term_new_with_alias_is_idempotent() {
    let repo = common::setup_repo();
    std::fs::write(repo.path().join("minds-terms.yaml"), "schema_version: '1'\nterms: []\n").unwrap();

    // First call: creates term + alias.
    let output1 = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "--json", "term", "new", "FooBar", "--alias", "foobar"])
        .output()
        .unwrap();
    assert!(output1.status.success());
    let v1: serde_json::Value = serde_json::from_slice(&output1.stdout).unwrap();
    assert_eq!(v1["data"]["created"], true);
    assert_eq!(v1["data"]["added_aliases"].as_array().unwrap().len(), 1);

    // Capture the file content after first write.
    let content1 = fs::read_to_string(repo.path().join("minds-terms.yaml")).unwrap();

    // Second call: identical invocation should be a no-op.
    let output2 = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "--json", "term", "new", "FooBar", "--alias", "foobar"])
        .output()
        .unwrap();
    assert!(output2.status.success());
    let v2: serde_json::Value = serde_json::from_slice(&output2.stdout).unwrap();
    assert_eq!(v2["data"]["created"], false, "second call must report created: false");
    assert_eq!(v2["data"]["added_aliases"].as_array().unwrap().len(), 0, "second call must report added_aliases: []");

    // Byte-stable.
    let content2 = fs::read_to_string(repo.path().join("minds-terms.yaml")).unwrap();
    assert_eq!(content1, content2, "second invocation must be byte-stable");
}

// ═══════════════════════════════════════════════════════════════════════════════
// T036 — Alias collision: alias already belongs to a different term
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn term_new_alias_collision_errors_with_owner_name() {
    let repo = common::setup_repo();
    std::fs::write(repo.path().join("minds-terms.yaml"), "schema_version: '1'\nterms: []\n").unwrap();

    // Create FooBar with alias foobar.
    Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "new", "FooBar", "--alias", "foobar"])
        .assert()
        .code(0);

    // Try to create BarBaz with the same alias foobar — must fail.
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "new", "BarBaz", "--alias", "foobar"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1), "alias collision must exit 1");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("FooBar"), "error must name FooBar as current owner of foobar: {stderr}");
    assert!(
        stderr.contains("conflicts") || stderr.to_lowercase().contains("foobar"),
        "error must mention the conflicting alias: {stderr}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// T038 — --dry-run reports implicit-create and alias-attach as planned actions
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn term_new_dry_run_reports_planned_actions() {
    let repo = common::setup_repo();
    std::fs::write(repo.path().join("minds-terms.yaml"), "schema_version: '1'\nterms: []\n").unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "new", "FooBar", "--alias", "foobar", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();

    // Must mention both creating and alias actions.
    assert!(
        stdout.contains("would create term") || stdout.contains("create term"),
        "dry-run must mention creation: {stdout}"
    );
    assert!(stdout.contains("foobar"), "dry-run must mention alias: {stdout}");
}

#[test]
fn term_new_dry_run_no_alias_reports_single_action() {
    let repo = common::setup_repo();
    std::fs::write(repo.path().join("minds-terms.yaml"), "schema_version: '1'\nterms: []\n").unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "new", "FooBar", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("would create term") || stdout.contains("create term"),
        "dry-run without alias should still mention creation: {stdout}"
    );
}
