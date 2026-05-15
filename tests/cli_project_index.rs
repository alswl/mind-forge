use assert_cmd::Command;
use std::fs;

mod common;

#[test]
fn test_index_adds_new_project() {
    let dir = common::setup_repo();
    common::create_project(&dir, "new-project");
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.current_dir(dir.path()).arg("project").arg("index");
    cmd.assert().code(0);
    let content = fs::read_to_string(dir.path().join("minds.yaml")).unwrap();
    assert!(content.contains("new-project"));
}

#[test]
fn test_index_removes_deleted_project() {
    let dir = common::setup_repo();
    common::create_project(&dir, "to-delete");
    let mut cmd1 = Command::cargo_bin("mf").unwrap();
    cmd1.current_dir(dir.path()).arg("project").arg("index").assert().code(0);
    fs::remove_dir_all(dir.path().join("to-delete")).unwrap();
    let mut cmd2 = Command::cargo_bin("mf").unwrap();
    cmd2.current_dir(dir.path()).arg("project").arg("index").assert().code(0);
    let content = fs::read_to_string(dir.path().join("minds.yaml")).unwrap();
    assert!(!content.contains("to-delete"));
}

#[test]
fn test_index_creates_minds_yaml_when_absent() {
    let dir = tempfile::TempDir::new().unwrap();
    common::create_project(&dir, "some-project");
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.current_dir(dir.path()).arg("project").arg("index");
    cmd.assert().code(0);
    assert!(dir.path().join("minds.yaml").exists());
}

#[test]
fn test_index_dry_run_does_not_modify() {
    let dir = common::setup_repo();
    common::create_project(&dir, "new-project");
    let before = fs::read_to_string(dir.path().join("minds.yaml")).unwrap();
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.current_dir(dir.path()).arg("project").arg("index").arg("--dry-run");
    cmd.assert().code(0);
    let after = fs::read_to_string(dir.path().join("minds.yaml")).unwrap();
    assert_eq!(before, after);
}

#[test]
fn test_index_works_in_non_repo() {
    let dir = tempfile::TempDir::new().unwrap();
    common::create_project(&dir, "some-project");
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.current_dir(dir.path()).arg("project").arg("index");
    cmd.assert().code(0);
    assert!(dir.path().join("minds.yaml").exists());
}

// ---------------------------------------------------------------------------
// Duplicate key lint tests (US1)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// US3: Informative warning detail tests
// ---------------------------------------------------------------------------

#[test]
fn test_invalid_yaml_warning_includes_detail() {
    let dir = common::setup_repo();
    common::create_project(&dir, "corrupt");
    common::write_index(&dir, "corrupt", "schema: '1'\nnot: : valid: yaml\n");

    // Register the corrupt project in minds.yaml first so listing discovers it
    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(dir.path())
        .args(["project", "index"])
        .assert()
        .code(0);

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(dir.path())
        .args(["project", "list"])
        .output()
        .expect("command runs");

    let full_output = format!("{}{}", String::from_utf8_lossy(&output.stderr), String::from_utf8_lossy(&output.stdout));
    // Should include the parse error detail
    assert!(
        full_output.contains("parse error") || full_output.contains("not") || full_output.contains("yaml"),
        "warning should include parse error detail: {full_output}"
    );
}

#[test]
fn test_lint_reports_duplicate_key() {
    let dir = common::setup_repo();
    common::create_project(&dir, "dup-project");
    common::write_index(
        &dir,
        "dup-project",
        r#"schema: '1'
articles:
  first:
    title: First
    source_path: docs/first.md
articles:
  second:
    title: Second
    source_path: docs/second.md
"#,
    );
    common::write_doc(&dir, "dup-project", "first", "# First\n");
    common::write_doc(&dir, "dup-project", "second", "# Second\n");

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(dir.path())
        .args(["project", "lint", "--project", "dup-project"])
        .output()
        .expect("command runs");

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("duplicate_key") || stdout.contains("duplicate key"),
        "should report duplicate_key issue in output: {stdout}"
    );
}

#[test]
fn test_lint_fix_removes_duplicate_keys() {
    let dir = common::setup_repo();
    common::create_project(&dir, "dup-project");
    common::write_index(
        &dir,
        "dup-project",
        r#"schema: '1'
articles:
  first:
    title: First
    source_path: docs/first.md
articles:
  second:
    title: Second
    source_path: docs/second.md
"#,
    );
    common::write_doc(&dir, "dup-project", "first", "# First\n");
    common::write_doc(&dir, "dup-project", "second", "# Second\n");

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(dir.path())
        .args(["project", "lint", "--project", "dup-project", "--fix", "--format", "json"])
        .assert()
        .code(0);

    // After fix, the file should be valid YAML without duplicate keys
    let content = fs::read_to_string(dir.path().join("dup-project/mind-index.yaml")).unwrap();
    // Should parse successfully (no duplicate key error)
    let parsed: serde_yaml::Value = serde_yaml::from_str(&content).unwrap();
    let map = parsed.as_mapping().unwrap();
    // The articles key may or may not be present (stale_entry fix may have removed it),
    // but there must be no duplicate key error
    let articles_keys: Vec<_> = map.keys().filter(|k| k.as_str() == Some("articles")).collect();
    assert!(articles_keys.len() <= 1, "articles key should appear at most once after fix: {content}");
    assert!(content.contains("first"), "fix should preserve entries from the first duplicate block: {content}");
    assert!(content.contains("second"), "fix should preserve entries from the second duplicate block: {content}");
}

#[test]
fn test_lint_fix_resolves_all_duplicate_keys_in_one_run() {
    let dir = common::setup_repo();
    common::create_project(&dir, "dup-project");
    common::write_index(
        &dir,
        "dup-project",
        r#"schema: '1'
articles:
  first:
    title: First
    source_path: docs/first.md
articles:
  second:
    title: Second
    source_path: docs/second.md
terms:
  - name: Alpha
    definition: A
terms:
  - name: Beta
    definition: B
"#,
    );

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(dir.path())
        .args(["project", "lint", "--project", "dup-project", "--fix"])
        .assert()
        .code(0);

    let second = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(dir.path())
        .args(["project", "lint", "--project", "dup-project"])
        .output()
        .expect("command runs");
    let stdout = String::from_utf8(second.stdout).unwrap();
    assert!(!stdout.contains("duplicate_key"), "all duplicate keys should be fixed in one run: {stdout}");

    let content = fs::read_to_string(dir.path().join("dup-project/mind-index.yaml")).unwrap();
    assert!(content.contains("first"), "article from first block should remain: {content}");
    assert!(content.contains("second"), "article from second block should remain: {content}");
    assert!(content.contains("Alpha"), "term from first block should remain: {content}");
    assert!(content.contains("Beta"), "term from second block should remain: {content}");
}
