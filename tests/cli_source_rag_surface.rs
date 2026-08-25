//! Contract tests for the terminal unified Source/RAG command surface.

mod common;

use common::embedding_provider::{
    Behavior, KEY_ENV, MockProvider, SECRET, configure_provider, provider_repo, provider_repo_for_embedding, report,
    run,
};

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
    let article = repo.path().join("projects/alpha/docs");
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

// ── Spec 075 US1 T014: rebuild warnings tell the truth (FR-008, FR-009) ──

/// A failure warning and the rebuild-completed statement must be reported
/// separately, and the completed statement must never appear when nothing
/// completed (`--dry-run`).
#[test]
fn rebuild_failure_warning_is_distinct_from_completion_statement() {
    let mock = MockProvider::start(Behavior::HttpError(500));
    let repo = provider_repo_for_embedding();
    configure_provider(repo.path(), &mock.endpoint);

    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--rebuild"], &[(KEY_ENV, SECRET)]);
    assert_eq!(code, 0, "rebuild must succeed despite per-item failures\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let r = report(&stdout);
    let failed = r["registrations_failed"].as_u64().unwrap_or(0);
    assert!(failed >= 1, "precondition: a real failure must be present\n{stdout}");

    assert!(
        stderr.contains(&format!("{failed}")) && stderr.contains("failed rebuild"),
        "stderr must name the failure count distinctly\nstderr:\n{stderr}"
    );
    // FR-008: the warning must name an actual remedy, not merely contain the
    // word "rebuild" (which "failed rebuild" itself already satisfies).
    assert!(
        stderr.contains("re-run `mf source sync --rebuild`"),
        "stderr must name the concrete re-run remedy\nstderr:\n{stderr}"
    );
}

/// `--dry-run` must claim nothing completed — it did not run a rebuild.
#[test]
fn rebuild_dry_run_claims_nothing_completed() {
    let repo = provider_repo();
    let (o, e, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "activation sync failed\n{o}\n{e}");

    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--rebuild", "--offline", "--dry-run"], &[]);
    assert_eq!(code, 0, "dry-run rebuild must succeed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(
        !stderr.contains("full re-index") && !stderr.contains("updated"),
        "dry-run must not claim a completed migration\nstderr:\n{stderr}"
    );
}
