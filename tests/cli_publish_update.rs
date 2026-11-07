//! Integration tests for `mf publish update` (feature 009-publish-mvp, US3).
//!
//! Success Criteria → Tests:
//!   SC-009 (idempotent repeat): `idempotent_repeat_invocation`
//!   SC-010 (composite key uniqueness): `composite_key_uniqueness`

use std::fs;

use assert_cmd::Command;

mod common;

const ARTICLE: &str = "my-article";

fn setup_repo_with_target() -> common::TempDir {
    let repo = common::setup_repo();
    let project_path = repo.path().join("my-project");
    fs::create_dir_all(&project_path).unwrap();
    fs::write(
        project_path.join("mind.yaml"),
        "schema_version: '1'\nproject:\n  name: my-project\npublish:\n  targets:\n    - name: local-blog\n      type: local\n      enabled: true\n      config:\n        path: /tmp/x\n",
    )
    .unwrap();
    fs::write(
        project_path.join("mind-index.yaml"),
        "schema_version: '1'\narticles:\n  - title: My Article\n    project: my-project\n    type: blog\n    article_path: docs/my-article.md\n    status: draft\n    created_at: '2026-05-07T00:00:00Z'\n    updated_at: '2026-05-07T00:00:00Z'\n",
    )
    .unwrap();
    repo
}

fn run_update(repo: &common::TempDir, args: &[&str]) -> std::process::Output {
    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(args)
        .output()
        .expect("command runs")
}

fn read_index_yaml(repo: &common::TempDir) -> String {
    fs::read_to_string(repo.path().join("my-project/mind-index.yaml")).unwrap()
}

fn read_index_bytes(repo: &common::TempDir) -> Vec<u8> {
    fs::read(repo.path().join("my-project/mind-index.yaml")).unwrap()
}

#[test]
fn creates_published_record_when_absent() {
    let repo = setup_repo_with_target();
    let out = run_update(
        &repo,
        &[
            "--output",
            "json",
            "publish",
            "update",
            ARTICLE,
            "--target",
            "local-blog",
            "--status",
            "published",
            "--target-url",
            "file:///tmp/x/my-article.md",
        ],
    );
    assert_eq!(out.status.code(), Some(0));

    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["status"], "ok");
    assert_eq!(v["data"]["action"], "created");
    let rec = &v["data"]["record"];
    assert_eq!(rec["path"], "docs/my-article.md");
    assert_eq!(rec["target_name"], "local-blog");
    assert_eq!(rec["status"], "published");
    let pub_at = rec["published_at"].as_str().expect("published_at is a string");
    chrono::DateTime::parse_from_rfc3339(pub_at).expect("RFC3339 published_at");

    let yaml = read_index_yaml(&repo);
    assert!(yaml.contains("publish_records:"), "index must contain publish_records");
}

#[test]
fn creates_draft_record_with_null_published_at() {
    let repo = setup_repo_with_target();
    let out = run_update(
        &repo,
        &["--output", "json", "publish", "update", ARTICLE, "--target", "local-blog", "--status", "draft"],
    );
    assert_eq!(out.status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["data"]["record"]["status"], "draft");
    assert_eq!(v["data"]["record"]["published_at"], serde_json::Value::Null);
}

#[test]
fn rejects_archived_creation_without_target_url() {
    let repo = setup_repo_with_target();
    let before = read_index_bytes(&repo);
    let out = run_update(
        &repo,
        &["--output", "json", "publish", "update", ARTICLE, "--target", "local-blog", "--set", "status=archived"],
    );
    assert_eq!(out.status.code(), Some(2));
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["error"]["kind"], "usage");
    assert!(v["error"]["hint"].as_str().unwrap_or("").contains("--target-url"), "hint must mention --target-url: {v}");
    assert_eq!(read_index_bytes(&repo), before, "index must be byte-unchanged on usage error");
}

#[test]
fn creates_archived_record_with_target_url() {
    let repo = setup_repo_with_target();
    let out = run_update(
        &repo,
        &[
            "--output",
            "json",
            "publish",
            "update",
            ARTICLE,
            "--target",
            "local-blog",
            "--status",
            "archived",
            "--target-url",
            "https://example.com/old",
        ],
    );
    assert_eq!(out.status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["data"]["record"]["status"], "archived");
    assert!(v["data"]["record"]["published_at"].is_string());
}

#[test]
fn updates_existing_record_status_only() {
    let repo = setup_repo_with_target();
    // Pre-create
    let _ = run_update(
        &repo,
        &[
            "publish",
            "update",
            ARTICLE,
            "--target",
            "local-blog",
            "--status",
            "published",
            "--target-url",
            "https://example.com/post",
        ],
    );

    // Update status only
    let out = run_update(
        &repo,
        &["--output", "json", "publish", "update", ARTICLE, "--target", "local-blog", "--status", "archived"],
    );
    assert_eq!(out.status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["data"]["action"], "updated");
    assert_eq!(v["data"]["record"]["status"], "archived");
    assert_eq!(v["data"]["record"]["target_url"], "https://example.com/post");
}

#[test]
fn updates_existing_record_target_url_only() {
    let repo = setup_repo_with_target();
    let _ = run_update(
        &repo,
        &[
            "publish",
            "update",
            ARTICLE,
            "--target",
            "local-blog",
            "--status",
            "published",
            "--target-url",
            "https://example.com/old",
        ],
    );

    let out = run_update(
        &repo,
        &[
            "--output",
            "json",
            "publish",
            "update",
            ARTICLE,
            "--target",
            "local-blog",
            "--target-url",
            "https://example.com/new",
        ],
    );
    assert_eq!(out.status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["data"]["action"], "updated");
    assert_eq!(v["data"]["record"]["status"], "published");
    assert_eq!(v["data"]["record"]["target_url"], "https://example.com/new");
}

#[test]
fn preserves_published_at_on_update() {
    let repo = setup_repo_with_target();
    let _ = run_update(
        &repo,
        &[
            "publish",
            "update",
            ARTICLE,
            "--target",
            "local-blog",
            "--status",
            "published",
            "--target-url",
            "https://example.com/post",
        ],
    );

    let v_first: serde_json::Value = serde_json::from_slice(
        &run_update(
            &repo,
            &[
                "--output",
                "json",
                "publish",
                "update",
                ARTICLE,
                "--target",
                "local-blog",
                "--target-url",
                "https://example.com/post",
            ],
        )
        .stdout,
    )
    .unwrap();
    let original_pub_at = v_first["data"]["record"]["published_at"].as_str().unwrap().to_string();

    std::thread::sleep(std::time::Duration::from_millis(1100));

    let out = run_update(
        &repo,
        &["--output", "json", "publish", "update", ARTICLE, "--target", "local-blog", "--status", "archived"],
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        v["data"]["record"]["published_at"].as_str().unwrap(),
        original_pub_at,
        "published_at must not be refreshed on update"
    );
}

#[test]
fn rejects_when_neither_status_nor_url_provided() {
    let repo = setup_repo_with_target();
    let before = read_index_bytes(&repo);
    let out = run_update(&repo, &["--output", "json", "publish", "update", ARTICLE, "--target", "local-blog"]);
    assert_eq!(out.status.code(), Some(2));
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["error"]["kind"], "usage");
    assert_eq!(read_index_bytes(&repo), before);
}

#[test]
fn rejects_when_article_not_in_index() {
    let repo = setup_repo_with_target();
    let out = run_update(
        &repo,
        &[
            "--output",
            "json",
            "publish",
            "update",
            "ghost-article",
            "--target",
            "local-blog",
            "--set",
            "status=published",
        ],
    );
    assert_eq!(out.status.code(), Some(1));
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["error"]["kind"], "not_found");
}

#[test]
fn rejects_when_target_not_in_config() {
    let repo = setup_repo_with_target();
    let out = run_update(
        &repo,
        &["--output", "json", "publish", "update", ARTICLE, "--target", "ghost-target", "--set", "status=published"],
    );
    assert_eq!(out.status.code(), Some(1));
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["error"]["kind"], "not_found");
}

#[test]
fn dry_run_does_not_write_index() {
    let repo = setup_repo_with_target();
    let before = read_index_bytes(&repo);
    let out = run_update(
        &repo,
        &[
            "--output",
            "json",
            "publish",
            "update",
            ARTICLE,
            "--target",
            "local-blog",
            "--status",
            "published",
            "--target-url",
            "https://example.com/post",
            "--dry-run",
        ],
    );
    assert_eq!(out.status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["data"]["dry_run"], true);
    assert_eq!(read_index_bytes(&repo), before, "mind-index.yaml byte-unchanged");
}

#[test]
fn idempotent_repeat_invocation() {
    let repo = setup_repo_with_target();
    let _ = run_update(
        &repo,
        &[
            "publish",
            "update",
            ARTICLE,
            "--target",
            "local-blog",
            "--status",
            "published",
            "--target-url",
            "https://example.com/post",
        ],
    );
    let after_first = read_index_bytes(&repo);

    let out = run_update(
        &repo,
        &[
            "--output",
            "json",
            "publish",
            "update",
            ARTICLE,
            "--target",
            "local-blog",
            "--status",
            "published",
            "--target-url",
            "https://example.com/post",
        ],
    );
    assert_eq!(out.status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["data"]["action"], "updated");

    assert_eq!(read_index_bytes(&repo), after_first, "second run must produce identical bytes (SC-009)");
}

#[test]
fn composite_key_uniqueness() {
    let repo = setup_repo_with_target();
    fs::write(
        repo.path().join("my-project/mind.yaml"),
        "schema_version: '1'\nproject:\n  name: my-project\npublish:\n  targets:\n    - name: local-blog\n      type: local\n      enabled: true\n      config:\n        path: /tmp/x\n    - name: blog2\n      type: local\n      enabled: true\n      config:\n        path: /tmp/y\n",
    )
    .unwrap();

    let _ = run_update(
        &repo,
        &[
            "publish",
            "update",
            ARTICLE,
            "--target",
            "local-blog",
            "--status",
            "published",
            "--target-url",
            "https://example.com/a",
        ],
    );
    let _ = run_update(
        &repo,
        &[
            "publish",
            "update",
            ARTICLE,
            "--target",
            "blog2",
            "--status",
            "published",
            "--target-url",
            "https://example.com/b",
        ],
    );
    let _ = run_update(
        &repo,
        &["publish", "update", ARTICLE, "--target", "local-blog", "--target-url", "https://example.com/a-updated"],
    );

    let yaml = read_index_yaml(&repo);
    let count_local = yaml.matches("target_name: local-blog").count();
    let count_blog2 = yaml.matches("target_name: blog2").count();
    assert_eq!(count_local, 1, "local-blog must appear exactly once: {yaml}");
    assert_eq!(count_blog2, 1, "blog2 must appear exactly once: {yaml}");
}

#[test]
fn text_output_renders_record_table() {
    let repo = setup_repo_with_target();
    let out = run_update(
        &repo,
        &[
            "publish",
            "update",
            ARTICLE,
            "--target",
            "local-blog",
            "--status",
            "published",
            "--target-url",
            "https://example.com/post",
        ],
    );
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("article      my-article"), "{stdout}");
    assert!(stdout.contains("target       local-blog"), "{stdout}");
    assert!(stdout.contains("action       created"), "{stdout}");
    assert!(stdout.contains("status       published"), "{stdout}");
    assert!(stdout.contains("target_url   https://example.com/post"), "{stdout}");
    assert!(stdout.contains("dry_run      false"), "{stdout}");
}

fn setup_article_shape(directory: bool) -> common::TempDir {
    let repo = setup_repo_with_target();
    let project = repo.path().join("my-project");
    if directory {
        let index = read_index_yaml(&repo).replace("docs/my-article.md", "docs/my-article");
        fs::write(project.join("mind-index.yaml"), index).unwrap();
        fs::create_dir_all(project.join("docs/my-article")).unwrap();
        fs::write(project.join("docs/my-article/01-opening.md"), "# Article\n").unwrap();
    } else {
        fs::create_dir_all(project.join("docs")).unwrap();
        fs::write(project.join("docs/my-article.md"), "# Article\n").unwrap();
    }
    repo
}

#[test]
fn canonical_selectors_create_and_update_the_same_record() {
    for (directory, path) in [(false, "docs/my-article.md"), (true, "docs/my-article")] {
        let repo = setup_article_shape(directory);
        let mut published_at = serde_json::Value::Null;
        for (i, selector) in [path, ARTICLE].into_iter().enumerate() {
            let out = run_update(
                &repo,
                &[
                    "publish",
                    "update",
                    selector,
                    "--target",
                    "local-blog",
                    "--set",
                    "status=published",
                    "--set",
                    "url=https://example.com/post",
                    "--json",
                ],
            );
            assert!(out.status.success(), "{selector}: {}", String::from_utf8_lossy(&out.stderr));
            let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
            assert_eq!(v["status"], "ok");
            assert_eq!(v["data"]["article"], selector);
            assert_eq!(v["data"]["action"], if i == 0 { "created" } else { "updated" });
            assert_eq!(v["data"]["record"]["path"], path);
            assert_eq!(v["data"]["record"]["target_url"], "https://example.com/post");
            if i == 0 {
                published_at = v["data"]["record"]["published_at"].clone();
                assert!(published_at.is_string());
            } else {
                assert_eq!(v["data"]["record"]["published_at"], published_at);
            }
        }
        assert_eq!(read_index_yaml(&repo).matches("target_name: local-blog").count(), 1);
    }
}

#[test]
fn canonical_selectors_dry_run_leaves_entire_tree_unchanged() {
    for (directory, path) in [(false, "docs/my-article.md"), (true, "docs/my-article")] {
        let repo = setup_article_shape(directory);
        for selector in [path, ARTICLE] {
            let before = common::snapshot_tree(repo.path());
            let out = run_update(
                &repo,
                &[
                    "publish",
                    "update",
                    selector,
                    "--target",
                    "local-blog",
                    "--set",
                    "status=published",
                    "--dry-run",
                    "--json",
                ],
            );
            assert!(out.status.success(), "{selector}: {}", String::from_utf8_lossy(&out.stderr));
            let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
            assert_eq!(v["data"]["record"]["path"], path);
            assert_eq!(v["data"]["dry_run"], true);
            common::assert_tree_unchanged(repo.path(), &before);
        }
    }
}

#[test]
fn canonical_selectors_preserve_target_and_field_validation() {
    for (directory, path) in [(false, "docs/my-article.md"), (true, "docs/my-article")] {
        let repo = setup_article_shape(directory);
        for (target, fields, code, kind) in [
            ("ghost-target", vec!["--set", "status=published"], 1, "not_found"),
            ("local-blog", vec![], 2, "usage"),
            ("local-blog", vec!["--set", "status=archived"], 2, "usage"),
            ("local-blog", vec!["--set", "url=https://example.com/post"], 2, "usage"),
            ("local-blog", vec!["--set", "unknown=value"], 2, "usage"),
        ] {
            let before = common::snapshot_tree(repo.path());
            let mut args = vec!["publish", "update", path, "--target", target, "--json"];
            args.extend(fields);
            let out = run_update(&repo, &args);
            assert_eq!(out.status.code(), Some(code), "{path}: {}", String::from_utf8_lossy(&out.stderr));
            let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
            assert_eq!(v["error"]["kind"], kind);
            common::assert_tree_unchanged(repo.path(), &before);
        }
    }
}
