//! CLI integration tests for project creation by path identity (US1).
//! Covers T016-T020.

use assert_cmd::Command;
use tempfile::TempDir;

mod common;

fn setup_flat_repo() -> TempDir {
    common::setup_repo()
}

fn mf(args: &[&str], repo: &TempDir) -> (String, String, i32) {
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap()])
        .args(args)
        .current_dir(repo.path())
        .output()
        .unwrap();
    (
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
        output.status.code().unwrap_or_default(),
    )
}

fn mf_json(args: &[&str], repo: &TempDir) -> (String, String, i32) {
    let mut full_args = vec!["--root", repo.path().to_str().unwrap(), "--json"];
    full_args.extend_from_slice(args);
    let output = Command::cargo_bin("mf").unwrap().args(&full_args).current_dir(repo.path()).output().unwrap();
    (
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
        output.status.code().unwrap_or_default(),
    )
}

// ── T016: Create project with Unicode path from repo root ───────────────

#[test]
fn project_new_unicode_path_from_repo_root() {
    let repo = setup_flat_repo();
    let (stdout, stderr, code) =
        mf_json(&["project", "new", "workspaces/E_团队周报/projects/2026-W21_iSee团队周报"], &repo);
    assert_eq!(code, 0, "stderr: {stderr}");

    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    assert_eq!(v["data"]["path"], "workspaces/E_团队周报/projects/2026-W21_iSee团队周报");
    assert_eq!(v["data"]["details"]["requested_path"], "workspaces/E_团队周报/projects/2026-W21_iSee团队周报");

    // Verify directory was created
    let project_dir = repo.path().join("workspaces/E_团队周报/projects/2026-W21_iSee团队周报");
    assert!(project_dir.join("mind.yaml").exists());
}

// ── T017: Create project cwd-relative ────────────────────────────────────

#[test]
fn project_new_cwd_relative() {
    let repo = setup_flat_repo();
    let ws_dir = repo.path().join("workspaces/E_团队周报/projects");
    std::fs::create_dir_all(&ws_dir).unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "--json", "project", "new", "2026-W21_iSee团队周报"])
        .current_dir(&ws_dir)
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(output.status.success(), "stderr: {stderr}");

    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    assert_eq!(v["data"]["path"], "workspaces/E_团队周报/projects/2026-W21_iSee团队周报");
    assert_eq!(v["data"]["details"]["requested_path"], "2026-W21_iSee团队周报");
}

// ── T018: Create project with emoji path ─────────────────────────────────

#[test]
fn project_new_emoji_path() {
    let repo = setup_flat_repo();
    let (stdout, stderr, code) = mf_json(&["project", "new", "workspaces/📊周报/projects/2026-W21_团队复盘🚀"], &repo);
    assert_eq!(code, 0, "stderr: {stderr}");

    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    assert!(v["data"]["path"].as_str().unwrap().contains("📊周报"));
    assert!(v["data"]["path"].as_str().unwrap().contains("团队复盘🚀"));

    let project_dir = repo.path().join("workspaces/📊周报/projects/2026-W21_团队复盘🚀");
    assert!(project_dir.join("mind.yaml").exists());
}

// ── T019: JSON envelope shape for project new success ────────────────────

#[test]
fn project_new_json_envelope() {
    let repo = setup_flat_repo();
    let (stdout, stderr, code) = mf_json(&["project", "new", "alpha"], &repo);
    assert_eq!(code, 0, "stderr: {stderr}");

    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    assert_eq!(v["command"], "mf");

    let data = &v["data"];
    assert!(data["details"]["path"].is_string(), "data.details.path missing: {data}");
    assert!(data["details"]["requested_path"].is_string(), "data.details.requested_path missing: {data}");
    assert!(data["details"]["created_at"].is_string(), "data.details.created_at missing: {data}");
    assert!(data["details"]["scaffolded"].is_array(), "data.details.scaffolded missing or not array: {data}");
}

// ── T020: Error cases ────────────────────────────────────────────────────

#[test]
fn project_new_rejects_path_escape() {
    let repo = setup_flat_repo();
    let (stdout, stderr, code) = mf_json(&["project", "new", "../outside"], &repo);
    assert_ne!(code, 0, "should fail: stdout={stdout} stderr={stderr}");

    // Error payload goes to stderr in JSON mode
    let error_output = if stderr.trim().is_empty() { stdout } else { stderr };
    let v: serde_json::Value = serde_json::from_str(&error_output).unwrap();
    assert_eq!(v["status"], "error");
    let err_msg = v["error"]["message"].as_str().unwrap();
    assert!(
        err_msg.contains("..") || err_msg.contains("outside") || err_msg.contains("escape"),
        "unexpected error message: {err_msg}"
    );
}

#[test]
fn project_new_rejects_duplicate_path() {
    let repo = setup_flat_repo();
    mf(&["project", "new", "dupe"], &repo);

    let (stdout, stderr, code) = mf_json(&["project", "new", "dupe"], &repo);
    assert_ne!(code, 0, "duplicate project should fail: stdout={stdout} stderr={stderr}");

    let error_output = if stderr.trim().is_empty() { stdout } else { stderr };
    let v: serde_json::Value = serde_json::from_str(&error_output).unwrap();
    assert_eq!(v["status"], "error");
}

#[test]
fn project_new_rejects_duplicate_nested_path() {
    let repo = setup_flat_repo();
    mf(&["project", "new", "workspaces/team/projects/my-report"], &repo);

    let (stdout, stderr, code) = mf_json(&["project", "new", "workspaces/team/projects/my-report"], &repo);
    assert_ne!(code, 0, "duplicate nested project should fail: stdout={stdout} stderr={stderr}");

    let error_output = if stderr.trim().is_empty() { stdout } else { stderr };
    let v: serde_json::Value = serde_json::from_str(&error_output).unwrap();
    assert_eq!(v["status"], "error");
}

#[test]
fn project_new_simple_name_still_works() {
    let repo = setup_flat_repo();
    let (stdout, stderr, code) = mf_json(&["project", "new", "simple-report"], &repo);
    assert_eq!(code, 0, "stderr: {stderr}");

    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    assert_eq!(v["data"]["path"], "simple-report");

    assert!(repo.path().join("simple-report/mind.yaml").exists());
}
