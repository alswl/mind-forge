//! CLI contract tests for source import provenance (spec 071, US1 FR-009a).
//!
//! `source add`/`new --article <path>` captures the originating article as
//! authoritative provenance on the source binding. Without `--article` the
//! project is still recorded and the article is null. An escaping `--article`
//! path is a usage error.

mod common;
use common::embedding_provider::{provider_repo, report, run};

fn add_source_file(repo: &tempfile::TempDir, rel: &str, body: &str) {
    let path = repo.path().join("projects/alpha").join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

#[test]
fn source_add_captures_originating_article() {
    let repo = provider_repo();
    add_source_file(&repo, "sources/file/imported.md", "# Imported\n\nBorrowed reference material.\n");

    let (o, e, code) = run(
        &repo,
        &[
            "source",
            "new",
            "sources/file/imported.md",
            "--project",
            "alpha",
            "--register-only",
            "--name",
            "imported",
            "--article",
            "outputs/2026-07/teleport.md",
        ],
        &[],
    );
    assert_eq!(code, 0, "source new --article failed\n{o}\n{e}");

    let (o, e, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "sync failed\n{o}\n{e}");

    let (stdout, _e, code) = run(&repo, &["source", "search", "imported", "--mode", "basic"], &[]);
    assert_eq!(code, 0);
    let r = report(&stdout);
    let hit = r["results"]
        .as_array()
        .and_then(|a| a.iter().find(|h| h["location"].as_str().is_some_and(|l| l.contains("imported.md"))))
        .expect("imported source hit");
    let imported_by = &hit["registrations"][0]["context"]["imported_by"];
    assert_eq!(imported_by["project"], "alpha");
    assert_eq!(imported_by["article"], "outputs/2026-07/teleport.md");
}

#[test]
fn source_add_without_article_records_project_only() {
    let repo = provider_repo();
    add_source_file(&repo, "sources/file/plain.md", "# Plain\n\nNo originating article.\n");

    let (o, e, code) = run(
        &repo,
        &["source", "new", "sources/file/plain.md", "--project", "alpha", "--register-only", "--name", "plain"],
        &[],
    );
    assert_eq!(code, 0, "source new failed\n{o}\n{e}");
    let (o, e, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "sync failed\n{o}\n{e}");

    let (stdout, _e, code) = run(&repo, &["source", "search", "plain", "--mode", "basic"], &[]);
    assert_eq!(code, 0);
    let r = report(&stdout);
    let hit = r["results"]
        .as_array()
        .and_then(|a| a.iter().find(|h| h["location"].as_str().is_some_and(|l| l.contains("plain.md"))))
        .expect("plain source hit");
    let imported_by = &hit["registrations"][0]["context"]["imported_by"];
    assert_eq!(imported_by["project"], "alpha");
    assert!(imported_by["article"].is_null(), "article must be null without --article\n{imported_by:#}");
}

#[test]
fn source_add_rejects_escaping_article_path() {
    let repo = provider_repo();
    add_source_file(&repo, "sources/file/escape.md", "# Escape\n");

    let (stdout, stderr, code) = run(
        &repo,
        &[
            "source",
            "new",
            "sources/file/escape.md",
            "--project",
            "alpha",
            "--register-only",
            "--name",
            "escape",
            "--article",
            "../../../etc/passwd",
        ],
        &[],
    );
    assert_eq!(code, 2, "escaping --article must be a usage error\nstdout:\n{stdout}\nstderr:\n{stderr}");
}
