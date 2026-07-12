//! `mf article block rm` — spec 064 Bug #20 (contracts/article-block-rm.md).

use assert_cmd::Command;
use std::fs;

mod common;

/// Create a directory article with the given blocks and a matching index entry.
fn setup_directory_article(repo: &common::TempDir, project: &str, article: &str, blocks: &[(&str, &str)]) {
    let project_path = repo.path().join(project);
    let article_dir = project_path.join("docs").join(article);
    fs::create_dir_all(&article_dir).unwrap();
    for (filename, content) in blocks {
        fs::write(article_dir.join(filename), content).unwrap();
    }
    let index_yaml = format!(
        "schema: '1'\narticles:\n  - title: {article}\n    project: {project}\n    type: blog\n    article_path: docs/{article}\n    status: draft\n    created_at: '2026-07-01T00:00:00Z'\n    updated_at: '2026-07-01T00:00:00Z'\n"
    );
    fs::write(project_path.join("mind-index.yaml"), index_yaml).unwrap();
}

fn mf() -> Command {
    Command::cargo_bin("mf").expect("binary exists")
}

#[test]
fn block_rm_happy_path_with_yes() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    setup_directory_article(
        &repo,
        "my-project",
        "my-article",
        &[("01-opening.md", "# Opening\n"), ("02-body.md", "## Body\n")],
    );

    mf().current_dir(repo.path().join("my-project"))
        .args(["article", "block", "rm", "my-article", "02-body", "--yes"])
        .assert()
        .success();

    let article_dir = repo.path().join("my-project/docs/my-article");
    assert!(!article_dir.join("02-body.md").exists(), "block file should be removed");
    assert!(article_dir.join("01-opening.md").exists(), "other blocks must be untouched");
}

#[test]
fn block_rm_dry_run_makes_no_changes() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    setup_directory_article(
        &repo,
        "my-project",
        "my-article",
        &[("01-opening.md", "# Opening\n"), ("02-body.md", "## Body\n")],
    );

    let output = mf()
        .current_dir(repo.path().join("my-project"))
        .args(["article", "block", "rm", "my-article", "02-body", "--dry-run"])
        .output()
        .expect("command runs");
    assert!(output.status.success());

    let article_dir = repo.path().join("my-project/docs/my-article");
    assert!(article_dir.join("02-body.md").exists(), "dry-run must not remove the file");
}

#[test]
fn block_rm_json_envelope_reflects_removal() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    setup_directory_article(
        &repo,
        "my-project",
        "my-article",
        &[("01-opening.md", "# Opening\n"), ("02-body.md", "## Body\n")],
    );

    let output = mf()
        .current_dir(repo.path().join("my-project"))
        .args(["article", "block", "rm", "my-article", "02-body", "--yes", "--json"])
        .output()
        .expect("command runs");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");

    assert_eq!(json["status"], "ok");
    let data = &json["data"];
    assert_eq!(data["kind"], "block");
    assert_eq!(data["removed"], true);
    assert_eq!(data["details"]["removed_filename"], "02-body.md");
    assert_eq!(data["details"]["remaining_blocks"], 1);
}

#[test]
fn block_rm_not_found_exits_1() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    setup_directory_article(
        &repo,
        "my-project",
        "my-article",
        &[("01-opening.md", "# Opening\n"), ("02-body.md", "## Body\n")],
    );

    let output = mf()
        .current_dir(repo.path().join("my-project"))
        .args(["article", "block", "rm", "my-article", "99-missing", "--yes"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(1), "block not found should be exit 1");
}

#[test]
fn block_rm_ambiguous_slug_exits_2() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    setup_directory_article(
        &repo,
        "my-project",
        "my-article",
        &[("01-notes.md", "# Notes A\n"), ("02-notes.md", "# Notes B\n"), ("03-third.md", "# Third\n")],
    );

    let output = mf()
        .current_dir(repo.path().join("my-project"))
        .args(["article", "block", "rm", "my-article", "notes", "--yes"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(2), "ambiguous slug should be a usage error");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("multiple blocks match"), "stderr: {stderr}");
}

#[test]
fn block_rm_single_file_article_exits_2() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_article_index(&repo, "my-project", "single-article");
    common::write_doc(&repo, "my-project", "single-article", "# Title\n\ncontent\n");

    let output = mf()
        .current_dir(repo.path().join("my-project"))
        .args(["article", "block", "rm", "single-article", "01-anything", "--yes"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(2), "single-file article should be a usage error");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not a directory article"), "stderr: {stderr}");
}

#[test]
fn block_rm_last_remaining_block_refused_with_hint() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    setup_directory_article(&repo, "my-project", "my-article", &[("01-opening.md", "# Opening\n")]);

    let output = mf()
        .current_dir(repo.path().join("my-project"))
        .args(["article", "block", "rm", "my-article", "01-opening", "--yes"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(2), "last-block removal should be a usage error");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("last remaining block"), "stderr: {stderr}");
    assert!(
        stderr.contains("article remove") || stderr.contains("article convert"),
        "stderr should hint at an alternative: {stderr}"
    );

    // File must be untouched.
    assert!(repo.path().join("my-project/docs/my-article/01-opening.md").exists());
}

#[test]
fn block_rm_non_tty_without_yes_or_force_errors() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    setup_directory_article(
        &repo,
        "my-project",
        "my-article",
        &[("01-opening.md", "# Opening\n"), ("02-body.md", "## Body\n")],
    );

    let output = mf()
        .current_dir(repo.path().join("my-project"))
        .args(["article", "block", "rm", "my-article", "02-body"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(2), "non-TTY without --yes/--force should be a usage error");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--yes"), "stderr should reference --yes: {stderr}");
    assert!(!stderr.contains("Confirm"), "should not show an interactive prompt in non-TTY: {stderr}");

    // Nothing removed.
    assert!(repo.path().join("my-project/docs/my-article/02-body.md").exists());
}
