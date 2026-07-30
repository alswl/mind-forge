use assert_cmd::Command;

mod common;

fn setup() -> common::TempDir {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    repo
}

fn mf(repo: &common::TempDir) -> Command {
    let mut command = Command::cargo_bin("mf").unwrap();
    command.args(["--root", repo.path().to_str().unwrap(), "--project", "alpha"]);
    command
}

#[test]
fn cjk_title_warns_and_explicit_slug_suppresses_warning() {
    let repo = setup();
    let output = mf(&repo).args(["article", "new", "中文标题"]).output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--slug"));

    let output = mf(&repo).args(["article", "new", "另一个标题", "--slug", "another-title"]).output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
    assert!(repo.path().join("alpha/docs/another-title").exists());
}

#[test]
fn ascii_title_has_no_slug_warning_and_dry_run_is_read_only() {
    let repo = setup();
    let before = common::snapshot_tree(repo.path());
    let output = mf(&repo).args(["article", "new", "Ascii Title", "--dry-run"]).output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
    common::assert_tree_unchanged(repo.path(), &before);
}
