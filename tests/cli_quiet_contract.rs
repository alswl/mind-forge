use assert_cmd::Command;
use std::fs;

mod common;

fn mf_quiet(repo: &common::TempDir) -> Command {
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.args(["--root", repo.path().to_str().unwrap(), "--quiet"]);
    cmd
}

fn setup_project_with_article() -> common::TempDir {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    fs::create_dir_all(project.join("docs")).unwrap();
    common::write_index(
        &repo,
        "alpha",
        r#"schema_version: '1'
articles:
  - title: 'Test Article'
    project: 'alpha'
    article_type: blog
    article_path: 'docs/test-article.md'
    status: draft
    created_at: '2026-05-07T00:00:00Z'
    updated_at: '2026-05-07T00:00:00Z'
"#,
    );
    repo
}

// ── Show: --quiet emits no stdout on success ────────────────────────────────

#[test]
fn quiet_project_show_no_stdout() {
    let repo = common::setup_repo();
    common::create_project(&repo, "demo");
    let output = mf_quiet(&repo).args(["project", "show", "demo"]).output().unwrap();
    assert!(
        output.status.success(),
        "exit_code={}, stderr={:?}",
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.trim().is_empty(), "stdout should be empty in quiet mode: {stdout}");
}

#[test]
fn quiet_term_show_no_stdout() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    common::write_term_index(&repo, "alpha", "term: RAG\n    definition: Retrieval-Augmented Generation\n");
    let output = mf_quiet(&repo).args(["term", "show", "RAG", "--project", "alpha"]).output().unwrap();
    assert!(
        output.status.success(),
        "exit_code={}, stderr={:?}",
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.trim().is_empty(), "stdout should be empty in quiet mode: {stdout}");
}

// ── Create: --quiet emits no stdout on success ──────────────────────────────

#[test]
fn quiet_project_new_no_stdout() {
    let repo = common::setup_repo();
    let output = mf_quiet(&repo).args(["project", "new", "demo"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.trim().is_empty(), "stdout should be empty in quiet mode: {stdout}");
}

#[test]
fn quiet_term_new_no_stdout() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let output = mf_quiet(&repo)
        .args(["term", "new", "test-term", "--project", "alpha", "--definition", "A test"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.trim().is_empty(), "stdout should be empty in quiet mode: {stdout}");
}

#[test]
fn quiet_article_new_no_stdout() {
    let repo = setup_project_with_article();
    let output =
        mf_quiet(&repo).args(["article", "new", "Quiet Article", "--project", "alpha", "--file"]).output().unwrap();
    assert!(
        output.status.success(),
        "exit_code={}, stderr={:?}",
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.trim().is_empty(), "stdout should be empty in quiet mode: {stdout}");
}

// ── Rename: --quiet emits no stdout on success ──────────────────────────────

#[test]
fn quiet_project_rename_no_stdout() {
    let repo = common::setup_repo();
    common::create_project(&repo, "demo");
    let output = mf_quiet(&repo).args(["project", "rename", "demo", "renamed"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.trim().is_empty(), "stdout should be empty in quiet mode: {stdout}");
}

// ── Remove: --quiet emits no stdout on success ──────────────────────────────

#[test]
fn quiet_project_remove_no_stdout() {
    let repo = common::setup_repo();
    common::create_project(&repo, "demo");
    let output = mf_quiet(&repo).args(["project", "remove", "demo", "--yes"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.trim().is_empty(), "stdout should be empty in quiet mode: {stdout}");
}

// ── Update: --quiet emits no stdout on success ──────────────────────────────

#[test]
fn quiet_source_update_no_stdout() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    // Create a source entry in the index
    common::write_index(
        &repo,
        "alpha",
        r#"schema_version: '1'
sources:
  - name: "my-source"
    type: pdf
    url: ~
    path: ~
    tags: []
    added_at: "2026-04-28T10:00:00Z"
    updated_at: "2026-04-28T10:00:00Z"
"#,
    );
    let output = mf_quiet(&repo)
        .args(["source", "update", "my-source", "--rename", "renamed-source", "--project", "alpha"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.trim().is_empty(), "stdout should be empty in quiet mode: {stdout}");
}

// ── Index: --quiet emits no stdout on success ───────────────────────────────

#[test]
fn quiet_article_index_no_stdout() {
    let repo = setup_project_with_article();
    let project = repo.path().join("alpha");
    fs::create_dir_all(project.join("docs")).unwrap();
    fs::write(project.join("docs/test-article.md"), "# Hi\n").unwrap();
    let output = mf_quiet(&repo).args(["article", "index", "--project", "alpha"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.trim().is_empty(), "stdout should be empty in quiet mode: {stdout}");
}
