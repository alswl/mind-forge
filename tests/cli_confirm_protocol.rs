use assert_cmd::Command;

mod common;

fn mf(repo: &common::TempDir) -> Command {
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.args(["--root", repo.path().to_str().unwrap()]);
    cmd
}

// ═══════════════════════════════════════════════════════════════════════════════
// T063: Confirmation protocol tests
// ═══════════════════════════════════════════════════════════════════════════════

// ── Non-TTY without flags → exit 1 ───────────────────────────────────────────

#[test]
fn remove_without_tty_exits_with_error() {
    let repo = common::setup_repo();
    common::create_project(&repo, "demo");
    // When not a TTY and no --yes/--force, should exit 1 with hint
    let output = mf(&repo).args(["project", "remove", "demo"]).output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("pass --yes to confirm"), "stderr: {stderr}");
    assert!(!stderr.contains("Confirm"), "should not show prompt in non-TTY: {stderr}");
    // Mutation should NOT have happened
    assert!(repo.path().join("demo").exists());
}

#[test]
fn archive_without_tty_exits_with_error() {
    let repo = common::setup_repo();
    common::create_project(&repo, "demo");
    let output = mf(&repo).args(["project", "archive", "demo"]).output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("pass --yes to confirm"), "stderr: {stderr}");
}

// ── Non-TTY with --yes → proceed ─────────────────────────────────────────────

#[test]
fn remove_with_yes_proceeds() {
    let repo = common::setup_repo();
    common::create_project(&repo, "demo");
    let output = mf(&repo).args(["project", "remove", "demo", "--yes"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("removed project:"), "stdout: {stdout}");
}

#[test]
fn archive_with_yes_proceeds() {
    let repo = common::setup_git_repo();
    common::create_project(&repo, "demo");
    // Stage project for git mv
    std::process::Command::new("git").args(["add", "demo/"]).current_dir(repo.path()).output().unwrap();
    let output = mf(&repo).args(["project", "archive", "demo", "--yes"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("archived project:"), "stdout: {stdout}");
}

// ── Non-TTY with --force + missing target → success no-op ────────────────────

#[test]
fn force_non_existent_success_noop() {
    let repo = common::setup_repo();
    common::create_project(&repo, "exists");
    // --force with non-existent target should succeed (no-op)
    let output = mf(&repo).args(["project", "remove", "nonexistent", "--force"]).output().unwrap();
    assert!(output.status.success());
}

#[test]
fn force_archive_non_existent_success_noop() {
    let repo = common::setup_repo();
    common::create_project(&repo, "exists");
    let output = mf(&repo).args(["project", "archive", "nonexistent", "--force"]).output().unwrap();
    assert!(output.status.success());
}

// ── Non-TTY with --yes + missing target → error ──────────────────────────────

#[test]
fn yes_non_existent_errors() {
    let repo = common::setup_repo();
    common::create_project(&repo, "exists");
    // --yes with non-existent target: safety check still applies → error
    let output = mf(&repo).args(["project", "remove", "nonexistent", "--yes"]).output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not found"), "stderr: {stderr}");
}

// ── Article/Source/Asset/Term remove without --yes in non-TTY ────────────────

#[test]
fn article_remove_without_yes_exits_error() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    std::fs::create_dir_all(repo.path().join("alpha/docs")).unwrap();
    let index = r#"schema_version: '1'
articles:
  - title: 'toremove'
    project: 'alpha'
    article_type: blog
    article_path: 'docs/toremove.md'
    status: draft
    created_at: '2026-05-07T00:00:00Z'
    updated_at: '2026-05-07T00:00:00Z'
"#;
    common::write_index(&repo, "alpha", index);
    let output = mf(&repo).args(["article", "remove", "docs/toremove.md", "--project", "alpha"]).output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("pass --yes to confirm"), "stderr: {stderr}");
}
