use assert_cmd::Command;
use std::fs;

mod common;

fn setup_repo_with_term() -> common::TempDir {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    fs::create_dir_all(project.join("docs")).unwrap();
    let index_yaml = r#"schema_version: '1'
terms:
  - term: Device
    corrections:
      - original: 机器
        correct: 装置
"#;
    common::write_index(&repo, "alpha", index_yaml);
    fs::write(project.join("docs").join("cjk.md"), "Python 机器 模型是热门。\n").unwrap();
    repo
}

fn mf(repo: &common::TempDir) -> Command {
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.args(["--root", repo.path().to_str().unwrap()]);
    cmd
}

// ── Equivalence: term fix ≡ term lint --fix ──

#[test]
fn fix_equivalent_to_lint_fix() {
    let repo1 = setup_repo_with_term();
    let repo2 = setup_repo_with_term();

    let fix_out = mf(&repo1).args(["term", "fix", "--project", "alpha", "docs/cjk.md", "-y"]).output().unwrap();
    let lint_fix_out =
        mf(&repo2).args(["term", "lint", "--project", "alpha", "docs/cjk.md", "--fix", "-y"]).output().unwrap();

    assert_eq!(fix_out.status.code(), lint_fix_out.status.code());
    assert_eq!(fix_out.stdout, lint_fix_out.stdout);
}

// ── Dry-run equivalence ──

#[test]
fn fix_dry_run_equivalent_to_lint_fix_dry_run() {
    let repo = setup_repo_with_term();

    let fix_out =
        mf(&repo).args(["term", "fix", "--project", "alpha", "docs/cjk.md", "--fix", "--dry-run"]).output().unwrap();
    let lint_out =
        mf(&repo).args(["term", "lint", "--project", "alpha", "docs/cjk.md", "--fix", "--dry-run"]).output().unwrap();

    assert_eq!(fix_out.status.code(), lint_out.status.code());
    assert_eq!(fix_out.stdout, lint_out.stdout);
}

// ── Legacy flag rejection ──

#[test]
fn fix_legacy_definition_flag_rejected() {
    let repo = setup_repo_with_term();
    let output = mf(&repo).args(["term", "fix", "--project", "alpha", "--definition", "X", "-y"]).output().unwrap();
    assert!(!output.status.success(), "legacy --definition should fail");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("update") || stderr.contains("unrecognized") || stderr.contains("error"),
        "error must hint at term update: stderr={stderr}"
    );
}

// ── Non-TTY safety gate ──

#[test]
fn fix_non_tty_without_yes_exits_2() {
    let repo = setup_repo_with_term();
    let output = mf(&repo).args(["term", "fix", "--project", "alpha", "docs/cjk.md"]).output().unwrap();
    assert!(!output.status.success(), "should exit non-zero");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--yes") || stderr.contains("--fix"), "error must mention --yes: stderr={stderr}");
}
