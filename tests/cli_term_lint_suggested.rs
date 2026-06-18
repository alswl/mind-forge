use assert_cmd::Command;
use std::fs;

mod common;

fn setup_repo_with_terms() -> common::TempDir {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    fs::create_dir_all(project.join("docs")).unwrap();
    repo
}

fn mf(repo: &common::TempDir) -> Command {
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.args(["--root", repo.path().to_str().unwrap()]);
    cmd
}

fn write_index(repo: &common::TempDir, yaml: &str) {
    common::write_index(repo, "alpha", yaml);
}

fn write_doc(repo: &common::TempDir, name: &str, content: &str) {
    let path = repo.path().join("alpha").join("docs").join(format!("{name}.md"));
    fs::write(path, content).unwrap();
}

// ── Scenario 1: fix: required (default) → term fix -y applies ──

#[test]
fn fix_required_applies_by_default() {
    let repo = setup_repo_with_terms();
    let index = r#"schema_version: '1'
terms:
  - term: RAG
    corrections:
      - original: rag
        correct: RAG
"#;
    write_index(&repo, index);
    write_doc(&repo, "ascii", "we use rag in production\n");

    let output = mf(&repo).args(["term", "fix", "--project", "alpha", "docs/ascii.md", "-y"]).output().unwrap();
    assert!(output.status.success(), "fix with default required should succeed");

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("✓ fixed"), "should show fixed: {stdout}");

    let doc = fs::read_to_string(repo.path().join("alpha/docs/ascii.md")).unwrap();
    assert!(doc.contains("RAG"), "file must contain corrected RAG: {doc}");
}

// ── Scenario 2: fix: suggested → term fix -y does NOT apply ──

#[test]
fn fix_suggested_not_applied_by_default() {
    let repo = setup_repo_with_terms();
    let index = r#"schema_version: '1'
terms:
  - term: RAG
    corrections:
      - original: rag
        correct: RAG
        fix: suggested
"#;
    write_index(&repo, index);
    write_doc(&repo, "ascii", "we use rag in production\n");

    let output = mf(&repo).args(["term", "fix", "--project", "alpha", "docs/ascii.md", "-y"]).output().unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("0 fixed"), "should show 0 fixed: {stdout}");

    let doc = fs::read_to_string(repo.path().join("alpha/docs/ascii.md")).unwrap();
    assert!(!doc.contains("RAG"), "file must NOT be changed for suggested: {doc}");
    assert!(doc.contains("rag"), "file must still contain rag: {doc}");
}

// ── Scenario 3: fix: suggested + --all → applies ──

#[test]
fn fix_suggested_applied_with_all_flag() {
    let repo = setup_repo_with_terms();
    let index = r#"schema_version: '1'
terms:
  - term: RAG
    corrections:
      - original: rag
        correct: RAG
        fix: suggested
"#;
    write_index(&repo, index);
    write_doc(&repo, "ascii", "we use rag in production\n");

    let output =
        mf(&repo).args(["term", "fix", "--project", "alpha", "docs/ascii.md", "--all", "-y"]).output().unwrap();
    assert!(output.status.success(), "--all fix should succeed");

    let doc = fs::read_to_string(repo.path().join("alpha/docs/ascii.md")).unwrap();
    assert!(doc.contains("RAG"), "--all must apply suggested: {doc}");
}

// ── Scenario 4: mixed required + suggested → only required applied by default ──

#[test]
fn mixed_required_and_suggested_partial_apply() {
    let repo = setup_repo_with_terms();
    let index = r#"schema_version: '1'
terms:
  - term: RAG
    corrections:
      - original: rag
        correct: RAG
        fix: suggested
      - original: mindrepo
        correct: Mind Repo
"#;
    write_index(&repo, index);
    write_doc(&repo, "mixed", "we use rag and mindrepo\n");

    let output = mf(&repo).args(["term", "fix", "--project", "alpha", "docs/mixed.md", "-y"]).output().unwrap();
    assert!(output.status.success());

    let doc = fs::read_to_string(repo.path().join("alpha/docs/mixed.md")).unwrap();
    assert!(!doc.contains("RAG"), "suggested rag should NOT be changed: {doc}");
    assert!(doc.contains("Mind Repo"), "required mindrepo should be changed: {doc}");
}

// ── Scenario 5: JSON envelope has fix_kind field ──

#[test]
fn json_finding_has_fix_kind() {
    let repo = setup_repo_with_terms();
    let index = r#"schema_version: '1'
terms:
  - term: RAG
    corrections:
      - original: rag
        correct: RAG
        fix: suggested
"#;
    write_index(&repo, index);
    write_doc(&repo, "ascii", "we use rag in production\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha", "--format", "json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"fix_kind\": \"suggested\""), "JSON must have suggested fix_kind: {stdout}");
}

// ── Scenario 6: --all without -y in non-TTY → exit non-zero ──

#[test]
fn all_without_yes_in_non_tty_exits_non_zero() {
    let repo = setup_repo_with_terms();
    let index = r#"schema_version: '1'
terms:
  - term: RAG
    corrections:
      - original: rag
        correct: RAG
"#;
    write_index(&repo, index);
    write_doc(&repo, "ascii", "we use rag\n");

    let output = mf(&repo).args(["term", "fix", "--project", "alpha", "docs/ascii.md", "--all"]).output().unwrap();
    assert!(!output.status.success(), "--all without -y should fail in non-TTY");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--yes") || stderr.contains("--fix"), "error must mention --yes: stderr={stderr}");
}
