//! US1 (spec 062): `mf article rm` must persist the index removal regardless of
//! the identifier form (title / article_path / index key, with or without `.md`),
//! report truthfully, and leave the index untouched on `--dry-run`.

use assert_cmd::Command;
use predicates::prelude::*;

mod common;

fn new_article(dir: &tempfile::TempDir, project: &str, title: &str) {
    Command::cargo_bin("mf")
        .unwrap()
        .current_dir(dir.path())
        .args(["article", "new", "-p", project, title])
        .assert()
        .code(0);
}

fn rm(dir: &tempfile::TempDir, project: &str, ident: &str) -> assert_cmd::assert::Assert {
    Command::cargo_bin("mf")
        .unwrap()
        .current_dir(dir.path())
        .args(["article", "rm", "-p", project, ident, "--yes"])
        .assert()
}

#[test]
fn rm_by_index_key_form_removes_index_entry() {
    let dir = common::setup_repo();
    common::create_project(&dir, "demo");
    new_article(&dir, "demo", "my-post");

    // Index key / article_path form (no `.md`).
    rm(&dir, "demo", "docs/my-post").code(0).stdout(predicates::str::contains("removed"));

    let map = common::read_index_articles_map(&dir, "demo");
    common::assert_no_article_key(&map, "docs/my-post");
}

#[test]
fn rm_by_md_suffixed_form_removes_index_entry() {
    let dir = common::setup_repo();
    common::create_project(&dir, "demo");
    new_article(&dir, "demo", "second");

    // `.md`-suffixed form must normalize to the same entry.
    rm(&dir, "demo", "docs/second.md").code(0);

    let map = common::read_index_articles_map(&dir, "demo");
    common::assert_no_article_key(&map, "docs/second");
}

#[test]
fn rm_by_title_form_still_works() {
    let dir = common::setup_repo();
    common::create_project(&dir, "demo");
    new_article(&dir, "demo", "third");

    rm(&dir, "demo", "third").code(0);

    let map = common::read_index_articles_map(&dir, "demo");
    common::assert_no_article_key(&map, "docs/third");
}

#[test]
fn rm_nonexistent_reports_not_found_and_never_success() {
    let dir = common::setup_repo();
    common::create_project(&dir, "demo");
    new_article(&dir, "demo", "present");

    rm(&dir, "demo", "does-not-exist")
        .failure()
        .stderr(predicates::str::contains("not found"))
        .stdout(predicates::str::contains("removed").not());

    // The real entry is untouched by the failed lookup.
    let map = common::read_index_articles_map(&dir, "demo");
    common::assert_article_path(&map, "docs/present", "docs/present");
}

#[test]
fn rm_dry_run_leaves_file_and_index_unchanged() {
    let dir = common::setup_repo();
    common::create_project(&dir, "demo");
    new_article(&dir, "demo", "keep");

    let before = std::fs::read_to_string(dir.path().join("demo").join("mind-index.yaml")).unwrap();

    Command::cargo_bin("mf")
        .unwrap()
        .current_dir(dir.path())
        .args(["article", "rm", "-p", "demo", "docs/keep", "--dry-run", "--yes"])
        .assert()
        .code(0)
        .stdout(predicates::str::contains("would remove"));

    let after = std::fs::read_to_string(dir.path().join("demo").join("mind-index.yaml")).unwrap();
    assert_eq!(before, after, "dry-run must not modify the index");
}

#[test]
fn rm_json_before_reflects_matched_entity() {
    let dir = common::setup_repo();
    common::create_project(&dir, "demo");
    new_article(&dir, "demo", "jsonpost");

    let out = Command::cargo_bin("mf")
        .unwrap()
        .current_dir(dir.path())
        .args(["--json", "article", "rm", "-p", "demo", "docs/jsonpost", "--yes"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON envelope");
    // `before` identity should reference the matched article_path.
    let dumped = v.to_string();
    assert!(dumped.contains("docs/jsonpost"), "JSON envelope should reference the matched entity: {dumped}");
}
