//! T056-T058 — Unified path resolution tests for `term lint` / `term fix` (US4).
//!
//! Three forms must all address the same file:
//!   1. `cd <root>; mf term lint sources/foo.md -p alpha`
//!   2. `cd <root>; mf term lint alpha/sources/foo.md`
//!   3. `cd <root>/alpha; mf term lint sources/foo.md`
//!
//! Absolute paths are used as-is. NotFound errors include both the literal
//! input and the attempted absolute path.

use assert_cmd::Command;
use std::fs;

mod common;

fn setup_with_global_term() -> (common::TempDir, std::path::PathBuf) {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    fs::create_dir_all(project.join("sources")).unwrap();
    fs::write(project.join("sources/foo.md"), "the mindrepo is here\n").unwrap();
    // Seed a global term so lint works without --project.
    fs::write(
        repo.path().join("minds-terms.yaml"),
        r#"schema_version: '1'
terms:
  - term: Mind Repo
    corrections:
      - original: mindrepo
        correct: Mind Repo
"#,
    )
    .unwrap();
    (repo, project)
}

fn mf(repo: &common::TempDir) -> Command {
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.args(["--root", repo.path().to_str().unwrap()]);
    cmd
}

// ═══════════════════════════════════════════════════════════════════════════════
// T056 — Three resolution forms address the same file
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn relative_with_p_flag_resolves() {
    let (repo, _project) = setup_with_global_term();
    let output = mf(&repo)
        .args(["term", "lint", "--project", "alpha", "sources/foo.md", "--include-suggested"])
        .output()
        .unwrap();
    assert!(output.status.success() || output.status.code() == Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("mindrepo"), "stdout should contain the finding: {stdout}");
}

#[test]
fn full_path_relative_to_repo_root_resolves() {
    let (repo, _project) = setup_with_global_term();
    let output = mf(&repo).args(["term", "lint", "alpha/sources/foo.md", "--include-suggested"]).output().unwrap();
    assert!(output.status.success() || output.status.code() == Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("mindrepo"), "stdout should contain the finding: {stdout}");
}

#[test]
fn cwd_based_resolution_finds_nearest_mind_yaml() {
    let (repo, project) = setup_with_global_term();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "lint", "sources/foo.md", "--include-suggested"])
        .current_dir(&project)
        .output()
        .unwrap();
    assert!(output.status.success() || output.status.code() == Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("mindrepo"), "stdout should contain the finding: {stdout}");
}

// ═══════════════════════════════════════════════════════════════════════════════
// T057 — Absolute paths are used as-is regardless of -p
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn absolute_paths_used_as_is() {
    let (repo, project) = setup_with_global_term();
    let abs_path = project.join("sources/foo.md");

    let output = mf(&repo).args(["term", "lint", &abs_path.to_string_lossy(), "--include-suggested"]).output().unwrap();
    assert!(output.status.success() || output.status.code() == Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("mindrepo"), "stdout should contain the finding: {stdout}");
}

// ═══════════════════════════════════════════════════════════════════════════════
// T058 — NotFound error includes both literal input and resolved path
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn not_found_error_includes_input_and_resolved_path() {
    let (repo, _project) = setup_with_global_term();
    let output = mf(&repo).args(["term", "lint", "--project", "alpha", "sources/missing.md"]).output().unwrap();
    assert!(!output.status.success(), "missing file should error");

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("sources/missing.md"), "error must include the literal input: {stderr}");
    assert!(
        stderr.contains("alpha/sources/missing.md") || stderr.contains("resolved"),
        "error must include resolved absolute path: {stderr}"
    );
}
