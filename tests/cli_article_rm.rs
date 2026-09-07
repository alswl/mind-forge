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

// ---------------------------------------------------------------------------
// Spec 079 US3 (#46) T025: `mf article rm`/`remove` must accept a bare slug,
// same as the full `docs/<slug>` identity — for both directory and
// single-file articles. Title is deliberately different from the slug so a
// bare-slug hit can only come from the new resolution path, not from
// accidentally matching `title`.
// ---------------------------------------------------------------------------

#[test]
fn rm_by_bare_slug_removes_index_entry() {
    let dir = common::setup_repo();
    common::create_project(&dir, "demo");
    Command::cargo_bin("mf")
        .unwrap()
        .current_dir(dir.path())
        .args(["article", "new", "-p", "demo", "Monthly Report", "--slug", "2026-09-monthly", "--file"])
        .assert()
        .code(0);

    // Bare slug — did not resolve before spec 079 US3 (title != slug here).
    rm(&dir, "demo", "2026-09-monthly").code(0).stdout(predicates::str::contains("removed"));

    let map = common::read_index_articles_map(&dir, "demo");
    common::assert_no_article_key(&map, "docs/2026-09-monthly");
}

#[test]
fn rm_by_bare_slug_matches_full_path_form_for_single_file_article() {
    let dir = common::setup_repo();
    common::create_project(&dir, "demo");
    Command::cargo_bin("mf")
        .unwrap()
        .current_dir(dir.path())
        .args(["article", "new", "-p", "demo", "Weekly Digest", "--slug", "weekly-digest", "--file"])
        .assert()
        .code(0);

    // Full-path form on a fresh copy of the same fixture, to compare outcomes.
    let dir2 = common::setup_repo();
    common::create_project(&dir2, "demo");
    Command::cargo_bin("mf")
        .unwrap()
        .current_dir(dir2.path())
        .args(["article", "new", "-p", "demo", "Weekly Digest", "--slug", "weekly-digest", "--file"])
        .assert()
        .code(0);

    rm(&dir, "demo", "weekly-digest").code(0);
    rm(&dir2, "demo", "docs/weekly-digest.md").code(0);

    let map1 = common::read_index_articles_map(&dir, "demo");
    let map2 = common::read_index_articles_map(&dir2, "demo");
    common::assert_no_article_key(&map1, "docs/weekly-digest");
    common::assert_no_article_key(&map2, "docs/weekly-digest");
}
