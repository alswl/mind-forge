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
    common::write_global_terms(
        &repo,
        r#"schema_version: '1'
terms:
  - term: Mind Repo
    corrections:
      - original: mindrepo
        correct: Mind Repo
"#,
    );
    (repo, project)
}

fn mf(repo: &common::TempDir) -> Command {
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.args(["--root", repo.path().to_str().unwrap()]);
    cmd
}

#[test]
fn external_directory_scans_same_files_as_individual_paths() {
    let (repo, _project) = setup_with_global_term();
    let (_holder, external) =
        common::scaffold_external_docs(&[("one.md", "mindrepo one\n"), ("two.md", "mindrepo two\n")]);

    let output = mf(&repo).args(["term", "lint", external.to_str().unwrap(), "--json"]).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["data"]["scanned_files"], 2);
    let paths: Vec<&str> =
        value["data"]["findings"].as_array().unwrap().iter().filter_map(|finding| finding["path"].as_str()).collect();
    assert_eq!(paths, vec!["one.md", "two.md"]);
}

#[test]
fn empty_external_directory_reports_no_eligible_files() {
    let (repo, _) = setup_with_global_term();
    let (_holder, external) = common::scaffold_external_docs(&[]);
    let output = mf(&repo).args(["term", "lint", external.to_str().unwrap()]).output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "No eligible files found.\n");
}

#[cfg(unix)]
#[test]
fn external_directory_fix_does_not_follow_symlink_escape() {
    use std::os::unix::fs::symlink;

    let (repo, _) = setup_with_global_term();
    let (_holder, external) = common::scaffold_external_docs(&[]);
    let outside = tempfile::NamedTempFile::new().unwrap();
    fs::write(outside.path(), "mindrepo outside\n").unwrap();
    symlink(outside.path(), external.join("escape.md")).unwrap();
    let output = mf(&repo).args(["term", "fix", external.to_str().unwrap(), "-y"]).output().unwrap();
    assert!(output.status.success());
    assert_eq!(fs::read_to_string(outside.path()).unwrap(), "mindrepo outside\n");
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
