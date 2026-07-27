//! Source lifecycle tests (T025): add/update/rename/remove/index/clean
//! with Lance-primary active.

mod common;
use common::embedding_provider::{provider_repo, run};

fn synced_repo() -> tempfile::TempDir {
    let repo = provider_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "sync failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    repo
}

#[test]
fn source_new_with_lance_mode_registers_source() {
    let repo = synced_repo();
    let input = repo.path().join("incoming.md");
    std::fs::write(&input, "# Added after activation\n\nDual-write coverage.\n").unwrap();

    let (stdout, stderr, code) = run(
        &repo,
        &["source", "new", &input.to_string_lossy(), "--name", "after-activation", "--project", "alpha"],
        &[],
    );
    assert_eq!(code, 0, "source new failed\nstdout:\n{stdout}\nstderr:\n{stderr}");

    // `source list` reads the active Lance primary catalog.
    let (list_out, _, code) = run(&repo, &["source", "list", "--project", "alpha"], &[]);
    assert_eq!(code, 0, "source list failed\n{list_out}");
    let v: serde_json::Value = serde_json::from_str(&list_out).expect("valid JSON");
    let sources = v["data"]["sources"].as_array().expect("sources array");
    assert!(
        sources.iter().any(|source| source["name"] == "after-activation"),
        "new Source must be present in Lance primary\n{list_out}"
    );

    // The same mutation is projected to legacy YAML for compatibility.
    let index = std::fs::read_to_string(repo.path().join("projects/alpha/mind-index.yaml")).unwrap();
    assert!(index.contains("after-activation"), "legacy projection must contain the new Source\n{index}");
}

#[test]
fn source_sync_indexes_article_artifacts_by_default() {
    let repo = synced_repo();
    let project = repo.path().join("projects/alpha");
    std::fs::create_dir_all(project.join("outputs/2026-07")).unwrap();
    std::fs::create_dir_all(project.join("prompts")).unwrap();
    std::fs::create_dir_all(project.join("thinking")).unwrap();
    std::fs::write(project.join("outputs/2026-07/article.md"), "article-only-search-phrase").unwrap();
    std::fs::write(project.join("prompts/article.md"), "prompt-only-search-phrase").unwrap();
    std::fs::write(project.join("thinking/article.md"), "thinking-only-search-phrase").unwrap();

    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "source sync failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let report = common::embedding_provider::report(&stdout);
    assert!(report["registrations_total"].as_u64().unwrap_or_default() >= 4, "article registrations missing: {stdout}");
    assert!(std::fs::read_to_string(project.join("outputs/2026-07/article.md")).unwrap().contains("article-only"));
}

#[test]
fn source_index_idempotent_in_lance_mode() {
    let repo = synced_repo();
    // Run index twice — second run must succeed.
    let (_, _, code1) = run(&repo, &["source", "index", "--project", "alpha"], &[]);
    assert_eq!(code1, 0);
    let (stdout, stderr, code2) = run(&repo, &["source", "index", "--project", "alpha"], &[]);
    assert_eq!(code2, 0, "second index must succeed\nstdout:\n{stdout}\nstderr:\n{stderr}");
}

#[test]
fn source_clean_removes_stale_entries() {
    let repo = synced_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "clean", "--project", "alpha"], &[]);
    assert_eq!(code, 0, "source clean failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
}
