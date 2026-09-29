//! Selector consistency for `mf article block move` and `block new` (spec 082, #57).

use std::fs;

use assert_cmd::Command;
mod common;

fn setup() -> common::TempDir {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    let article = repo.path().join("my-project/docs/x");
    fs::create_dir_all(&article).unwrap();
    fs::write(article.join("01-opening.md"), "# Opening\n").unwrap();
    fs::write(article.join("02-body.md"), "# Body\n").unwrap();
    fs::write(article.join("03-progress.md"), "# Progress\n").unwrap();
    fs::write(
        repo.path().join("my-project/mind-index.yaml"),
        "schema: '1'\narticles:\n  - title: X\n    project: my-project\n    type: blog\n    article_path: docs/x\n    status: draft\n    created_at: '2026-07-01T00:00:00Z'\n    updated_at: '2026-07-01T00:00:00Z'\n",
    ).unwrap();
    repo
}

fn mf(repo: &common::TempDir, args: &[&str]) -> assert_cmd::assert::Assert {
    Command::cargo_bin("mf").unwrap().current_dir(repo.path().join("my-project")).args(args).assert()
}

fn order(repo: &common::TempDir) -> Vec<String> {
    let mut names: Vec<_> = fs::read_dir(repo.path().join("my-project/docs/x"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn move_after_accepts_filename_numbered_stem_and_slug() {
    let selectors = ["01-opening.md", "01-opening", "opening"];
    let mut results = Vec::new();
    for selector in selectors {
        let repo = setup();
        mf(&repo, &["article", "block", "move", "docs/x", "03-progress", "--after", selector]).success();
        results.push(order(&repo));
    }
    assert!(results.windows(2).all(|pair| pair[0] == pair[1]), "results differ: {results:?}");
    assert_eq!(results[0], ["01-opening.md", "02-progress.md", "03-body.md"]);
}

#[test]
fn move_accepts_completed_block_path() {
    let repo = setup();
    mf(&repo, &["article", "block", "move", "docs/x", "docs/x/03-progress.md", "--after", "01-opening"]).success();
    assert_eq!(order(&repo), ["01-opening.md", "02-progress.md", "03-body.md"]);
}

#[test]
fn move_unresolvable_block_is_not_internal_and_has_hint() {
    let repo = setup();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .current_dir(repo.path().join("my-project"))
        .args(["--output", "json", "article", "block", "move", "docs/x", "missing"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(value["error"]["kind"], "not_found");
    assert!(!value["error"]["hint"].as_str().unwrap_or_default().is_empty());
    assert!(!value.to_string().contains("os error"));
}

#[test]
fn move_rejects_completed_path_from_another_article() {
    let repo = setup();
    fs::create_dir_all(repo.path().join("my-project/docs/y")).unwrap();
    fs::write(repo.path().join("my-project/docs/y/01-opening.md"), "# Other\n").unwrap();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .current_dir(repo.path().join("my-project"))
        .args(["--output", "json", "article", "block", "move", "docs/x", "docs/y/01-opening.md"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(value["error"]["kind"], "not_found");
}

#[test]
fn new_after_uses_the_same_numbered_stem_resolver() {
    let repo = setup();
    mf(&repo, &["article", "block", "new", "docs/x", "closing", "--after", "01-opening"]).success();
    assert!(repo.path().join("my-project/docs/x/02-closing.md").exists());
}

#[test]
fn existing_block_order_and_numbering_are_preserved_for_valid_anchor() {
    let repo = setup();
    mf(&repo, &["article", "block", "move", "docs/x", "03-progress", "--after", "01-opening.md"]).success();
    assert_eq!(order(&repo), ["01-opening.md", "02-progress.md", "03-body.md"]);
}

#[test]
fn ambiguous_after_anchor_has_disambiguation_hint() {
    let repo = setup();
    fs::write(repo.path().join("my-project/docs/x/04-opening.md"), "# Again\n").unwrap();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .current_dir(repo.path().join("my-project"))
        .args(["--output", "json", "article", "block", "new", "docs/x", "closing", "--after", "opening"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(value["error"]["kind"], "usage");
    assert!(value["error"]["hint"].as_str().unwrap_or_default().contains("filename"));
}
