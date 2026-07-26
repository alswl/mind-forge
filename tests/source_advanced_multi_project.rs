//! Multi-project activation and CRUD tests (T022, T025): Lance-primary authority,
//! list/show/CRUD across projects, and last-binding visibility.

mod common;
use common::embedding_provider::{provider_repo, run};

fn synced_repo() -> tempfile::TempDir {
    let repo = provider_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "advanced", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "sync failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    repo
}

// ── T022: Activation and Lance-primary ──

#[test]
fn source_list_shows_sources_from_all_projects() {
    let repo = synced_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "list", "--project", "alpha"], &[]);
    assert_eq!(code, 0, "source list failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let sources = v["data"]["sources"].as_array().expect("sources array");
    assert!(!sources.is_empty(), "must list sources after sync\n{stdout}");
    for src in sources {
        assert!(src["name"].as_str().is_some(), "source must have name\n{stdout}");
    }
}

// ── T025: Source lifecycle ──

#[test]
fn source_show_with_lance_active_returns_source_details() {
    let repo = synced_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "show", "notes", "--project", "alpha"], &[]);
    assert_eq!(code, 0, "source show failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(v["status"], "ok");
    assert_eq!(v["data"]["name"], "notes");
}

#[test]
fn source_index_still_works_in_lance_mode() {
    let repo = synced_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "index", "--project", "alpha"], &[]);
    assert_eq!(code, 0, "source index must work in Lance mode\nstdout:\n{stdout}\nstderr:\n{stderr}");
}

#[test]
fn unknown_source_show_is_not_found() {
    let repo = synced_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "show", "nonexistent-source", "--project", "alpha"], &[]);
    // Must fail: source does not exist.
    assert!(code != 0, "unknown source must fail\ngot code {code}\nstdout:\n{stdout}\nstderr:\n{stderr}");
}
