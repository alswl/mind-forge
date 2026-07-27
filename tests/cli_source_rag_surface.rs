//! Contract tests for the terminal unified Source/RAG command surface.

mod common;

use common::embedding_provider::{provider_repo, run};

#[test]
fn canonical_source_help_exposes_rag_commands() {
    let repo = provider_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "--help"], &[]);
    assert_eq!(code, 0, "source help failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(stdout.contains("sync"));
    assert!(stdout.contains("admin"));
    assert!(!stdout.contains("advanced"));
}

#[test]
fn canonical_search_help_has_no_mode_switch() {
    let repo = provider_repo();
    let (stdout, stderr, code) = run(&repo, &["search", "--help"], &[]);
    assert_eq!(code, 0, "search help failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(!stdout.contains("--mode"));
}

#[test]
fn canonical_search_covers_source_and_article_content() {
    let repo = provider_repo();
    let article = repo.path().join("projects/alpha/outputs/2026-07");
    std::fs::create_dir_all(&article).unwrap();
    std::fs::write(article.join("rag-surface.md"), "article-rag-surface-phrase").unwrap();
    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "sync failed\nstdout:\n{stdout}\nstderr:\n{stderr}");

    let (stdout, stderr, code) = run(&repo, &["search", "article-rag-surface-phrase"], &[]);
    assert_eq!(code, 0, "canonical search failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let report = common::embedding_provider::report(&stdout);
    assert!(report["results"].as_array().is_some_and(|results| !results.is_empty()), "article hit missing: {stdout}");
}

#[test]
fn sync_initializes_a_legacy_only_repository() {
    let repo = provider_repo();
    let cache = repo.path().join(".mind-forge/cache/source/advanced");
    if cache.exists() {
        std::fs::remove_dir_all(&cache).unwrap();
    }
    std::fs::write(
        repo.path().join("minds.yaml"),
        "schema_version: '1'\nprojects:\n  - name: alpha\n    path: ./projects/alpha\n    created_at: \"2026-07-17T08:00:00Z\"\n    archived_at: ~\n",
    )
    .unwrap();

    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--offline", "--dry-run"], &[]);
    assert_eq!(code, 0, "legacy dry-run failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(stdout.contains("activation") || stdout.contains("registrations"), "dry-run preview missing: {stdout}");

    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "legacy activation failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(repo.path().join(".mind-forge/cache/source/advanced").exists());
}

#[test]
fn promoted_trace_uses_canonical_command() {
    let repo = provider_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "trace"], &[]);
    assert_eq!(code, 0, "trace failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(stdout.contains("links") || stdout.contains("notes.md"), "trace payload missing: {stdout}");
}

#[test]
fn removed_advanced_surface_is_rejected() {
    let repo = provider_repo();
    for args in [
        &["source", "advanced", "sync"][..],
        &["source", "advanced", "enable"][..],
        &["source", "advanced", "disable"][..],
        &["source", "advanced", "legacy", "export"][..],
        &["source", "advanced", "model", "install"][..],
        &["source", "advanced", "skill-install"][..],
        &["source", "advanced", "enrich", "list"][..],
    ] {
        let (stdout, stderr, code) = run(&repo, args, &[]);
        assert_eq!(code, 2, "removed command accepted: {args:?}\nstdout:\n{stdout}\nstderr:\n{stderr}");
    }
}
