//! Catalog tests (T011): legacy/Lance authority, active vs archived projects,
//! cwd independence, and no silent fallback.

mod common;
use common::embedding_provider::{provider_repo, run};

fn enabled_repo() -> tempfile::TempDir {
    provider_repo()
}

#[test]
fn lance_mode_catalog_lists_registrations_from_lance() {
    let repo = enabled_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "list", "--project", "alpha"], &[]);
    assert_eq!(code, 0, "source list failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let sources = v["data"]["sources"].as_array().expect("sources array");
    // After enable but before sync, registrations are imported from legacy.
    assert!(!sources.is_empty(), "Lance catalog must list registrations\n{stdout}");
}

#[test]
fn lance_mode_source_show_returns_registration_details() {
    let repo = enabled_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "show", "notes", "--project", "alpha"], &[]);
    assert_eq!(code, 0, "source show failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(v["status"], "ok");
    assert!(v["data"]["name"].as_str().is_some(), "must have name field\n{stdout}");
}

#[test]
fn cwd_independent_source_list_uses_repo_root_not_cwd() {
    let repo = enabled_repo();
    // Run from a subdirectory — should still find sources via --root.
    let mut cmd = assert_cmd::Command::cargo_bin("mf").expect("mf binary");
    let subdir = repo.path().join("projects/alpha/sources");
    std::fs::create_dir_all(&subdir).unwrap();
    cmd.arg("--root").arg(repo.path()).current_dir(&subdir).args([
        "--output",
        "json",
        "source",
        "list",
        "--project",
        "alpha",
    ]);
    let output = cmd.output().expect("run mf");
    assert_eq!(output.status.code().unwrap_or(-1), 0);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert!(!v["data"]["sources"].as_array().unwrap().is_empty(), "list from subdir must work\n{stdout}");
}
