//! CLI contract tests for sync coverage reporting and new kinds (spec 071, US2).
//!
//! `mf source sync --format json` reports per-kind `coverage` (including the new
//! `project` and `term` kinds) and item-by-item `skipped_items`, so nothing is
//! silently dropped and coverage is auditable. `--dry-run` reports without
//! writing. `mf search` can hit project goals and term definitions.

mod common;
use common::embedding_provider::{provider_repo, report, run};

/// Extend the activated repo with a project goal and a global term so the new
/// `project`/`term` kinds have content to index.
fn repo_with_project_and_term() -> tempfile::TempDir {
    let repo = provider_repo();
    let alpha = repo.path().join("projects/alpha");
    std::fs::write(alpha.join("mind.yaml"), "schema_version: '1'\ngoal: Investigate superconductivity pathways\n")
        .unwrap();
    std::fs::write(
        repo.path().join("minds-terms.yaml"),
        "schema_version: '1'\nterms:\n  - term: Qubit\n    definition: A two-level quantum system used for computation\n",
    )
    .unwrap();
    let (o, e, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "sync failed\n{o}\n{e}");
    repo
}

fn coverage_for<'a>(data: &'a serde_json::Value, kind: &str) -> Option<&'a serde_json::Value> {
    data["coverage"].as_array()?.iter().find(|c| c["kind"] == kind)
}

#[test]
fn coverage_reports_project_and_term_kinds() {
    let repo = repo_with_project_and_term();
    // Re-sync reports coverage over the already-indexed corpus.
    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "sync failed\n{stdout}\n{stderr}");
    let data = report(&stdout);

    for kind in ["project", "term"] {
        let cov = coverage_for(&data, kind);
        assert!(cov.is_some(), "coverage must include kind '{kind}'\n{data:#}");
    }
    // indexed + skipped equals discovered total per kind (no silent loss).
    for cov in data["coverage"].as_array().expect("coverage array") {
        assert!(cov["indexed"].is_u64() && cov["skipped"].is_u64(), "coverage counts present\n{cov:#}");
    }
}

#[test]
fn search_hits_project_goal_and_term_definition() {
    let repo = repo_with_project_and_term();

    let (stdout, _e, code) = run(&repo, &["source", "search", "project:alpha", "--mode", "basic"], &[]);
    assert_eq!(code, 0);
    let r = report(&stdout);
    assert!(
        r["results"].as_array().is_some_and(|a| a.iter().any(|h| h["source_type"] == "project")),
        "search must surface the project kind\n{stdout}"
    );

    let (stdout, _e, code) = run(&repo, &["source", "search", "term:Qubit", "--mode", "basic"], &[]);
    assert_eq!(code, 0);
    let r = report(&stdout);
    assert!(
        r["results"].as_array().is_some_and(|a| a.iter().any(|h| h["source_type"] == "term")),
        "search must surface the term kind\n{stdout}"
    );
}

#[test]
fn empty_item_is_reported_skipped_not_silently_dropped() {
    let repo = provider_repo();
    // A zero-byte source file yields no chunks.
    let sources = repo.path().join("projects/alpha/sources/file");
    std::fs::write(sources.join("empty.md"), "").unwrap();
    let (o, e, c) = run(
        &repo,
        &[
            "source",
            "new",
            "sources/file/empty.md",
            "--project",
            "alpha",
            "--register-only",
            "--no-index",
            "--name",
            "empty",
        ],
        &[],
    );
    assert_eq!(c, 0, "register empty source failed\n{o}\n{e}");

    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "sync failed\n{stdout}\n{stderr}");
    let data = report(&stdout);
    let skipped = data["skipped_items"].as_array().unwrap_or_else(|| panic!("no skipped_items\nfull data:\n{data:#}"));
    assert!(
        skipped
            .iter()
            .any(|s| s["location"].as_str().is_some_and(|l| l.contains("empty.md")) && s["reason"] == "empty"),
        "empty source must be reported as skipped with reason 'empty'\n{data:#}"
    );
}

#[test]
fn dry_run_reports_without_writing() {
    let repo = repo_with_project_and_term();
    let before = std::fs::metadata(repo.path().join("minds.yaml")).unwrap().modified().unwrap();

    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--dry-run", "--offline"], &[]);
    assert_eq!(code, 0, "dry-run failed\n{stdout}\n{stderr}");
    let data = report(&stdout);
    assert_eq!(data["dry_run"], true, "dry_run flag must be set\n{data:#}");

    let after = std::fs::metadata(repo.path().join("minds.yaml")).unwrap().modified().unwrap();
    assert_eq!(before, after, "dry-run must not modify the manifest");
}
