use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;

mod common;

fn setup() -> common::TempDir {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    common::create_project(&repo, "beta");
    let alpha = repo.path().join("alpha");
    std::fs::create_dir_all(alpha.join("sources/file")).unwrap();
    std::fs::create_dir_all(alpha.join("assets")).unwrap();
    std::fs::create_dir_all(alpha.join("docs/post")).unwrap();
    std::fs::create_dir_all(alpha.join("prompts")).unwrap();
    std::fs::create_dir_all(alpha.join("thinking")).unwrap();
    std::fs::write(alpha.join("sources/file/note.md"), "note").unwrap();
    std::fs::write(alpha.join("assets/diagram.png"), b"png").unwrap();
    std::fs::write(alpha.join("docs/post/01-main.md"), "# Post\n").unwrap();
    std::fs::write(alpha.join("prompts/post.md"), "article: docs/post\n").unwrap();
    std::fs::write(alpha.join("thinking/post.md"), "# thinking\n").unwrap();
    std::fs::write(
        alpha.join("mind-index.yaml"),
        "schema: '1'\nsources:\n  - name: note\n    type: file\n    path: sources/file/note.md\nassets:\n  - name: diagram\n    type: image\n    path: assets/diagram.png\n    size: 3\n    hash: ''\n    tags: []\n    added_at: ''\narticles:\n  - title: Post\n    project: alpha\n    type: blank\n    article_path: docs/post\n    status: draft\n    created_at: ''\n    updated_at: ''\nprompts:\n  - path: prompts/post.md\n    article: docs/post\nthinking:\n  - path: thinking/post.md\n    article: docs/post\n",
    )
    .unwrap();
    repo
}

fn mf(repo: &common::TempDir, project: &str) -> Command {
    let mut command = Command::cargo_bin("mf").unwrap();
    command.args(["--root", repo.path().to_str().unwrap(), "--project", project]);
    command
}

#[test]
fn moves_source_asset_and_article_between_projects() {
    let repo = setup();
    let output =
        mf(&repo, "alpha").args(["source", "move", "note", "--to-project", "beta", "--json"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(repo.path().join("beta/sources/file/note.md").exists());
    assert!(!repo.path().join("alpha/sources/file/note.md").exists());
    assert_eq!(serde_json::from_slice::<Value>(&output.stdout).unwrap()["data"]["new_path"], "sources/file/note.md");

    let output =
        mf(&repo, "alpha").args(["asset", "move", "diagram", "--to-project", "beta", "--json"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(repo.path().join("beta/assets/diagram.png").exists());

    let output =
        mf(&repo, "alpha").args(["article", "move", "Post", "--to-project", "beta", "--json"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(repo.path().join("beta/docs/post/01-main.md").exists());
    assert!(repo.path().join("beta/prompts/post.md").exists());
    assert!(repo.path().join("beta/thinking/post.md").exists());
    let data = serde_json::from_slice::<Value>(&output.stdout).unwrap();
    assert_eq!(data["data"]["moved_prompts"][0], "prompts/post.md");
    assert_eq!(data["data"]["moved_thinking"][0], "thinking/post.md");
}

#[test]
fn move_dry_run_and_conflicts_are_non_destructive() {
    let repo = setup();
    let before = common::snapshot_tree(repo.path());
    let output = mf(&repo, "alpha")
        .args(["article", "move", "Post", "--to-project", "beta", "--dry-run", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    common::assert_tree_unchanged(repo.path(), &before);

    std::fs::create_dir_all(repo.path().join("beta/docs/post")).unwrap();
    let output = mf(&repo, "alpha").args(["article", "move", "Post", "--to-project", "beta"]).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("destination"));
    assert!(repo.path().join("alpha/docs/post/01-main.md").exists());
}

#[test]
fn prompt_and_thinking_views_are_sorted_and_read_only() {
    let repo = setup();
    let before = common::snapshot_tree(repo.path());
    let output = mf(&repo, "alpha").args(["prompt", "list"]).output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("prompts/post.md"));
    let json = mf(&repo, "alpha").args(["thinking", "list", "--json"]).output().unwrap();
    assert!(json.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&json.stdout).unwrap()["data"]["thinking"][0]["path"],
        "thinking/post.md"
    );
    common::assert_tree_unchanged(repo.path(), &before);

    let missing = mf(&repo, "alpha").args(["prompt", "show", "missing"]).output().unwrap();
    assert_eq!(missing.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&missing.stderr).contains("prompt list"));
}

#[test]
fn same_project_move_is_rejected() {
    let repo = setup();
    mf(&repo, "alpha")
        .args(["asset", "move", "diagram", "--to-project", "alpha"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("must differ"));
}

#[test]
fn directory_block_new_move_and_renumber_are_json_and_dry_run_safe() {
    let repo = setup();
    let output = mf(&repo, "alpha")
        .args(["article", "block", "new", "Post", "second", "--after", "01-main.md", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(repo.path().join("alpha/docs/post/02-second.md").exists());
    let data = serde_json::from_slice::<Value>(&output.stdout).unwrap();
    assert_eq!(data["data"]["details"]["new_path"], "docs/post/02-second.md");

    let before = common::snapshot_tree(repo.path());
    let output = mf(&repo, "alpha")
        .args(["article", "block", "move", "Post", "second", "--after", "01-main.md", "--dry-run", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    common::assert_tree_unchanged(repo.path(), &before);

    let output =
        mf(&repo, "alpha").args(["article", "block", "renumber", "Post", "--start", "3", "--json"]).output().unwrap();
    assert!(output.status.success());
    assert!(repo.path().join("alpha/docs/post/03-main.md").exists());
    assert!(repo.path().join("alpha/docs/post/04-second.md").exists());
}
