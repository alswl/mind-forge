use assert_cmd::Command;
use tempfile::TempDir;

mod common;

fn setup() -> TempDir {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    common::create_project(&repo, "beta");
    // Populate minds.yaml with project entries
    let manifest = r#"schema_version: '1'
projects_dir: .
projects:
  - name: alpha
    path: ./alpha
    created_at: '2026-05-08T10:00:00Z'
  - name: beta
    path: ./beta
    created_at: '2026-05-08T11:00:00Z'
"#;
    std::fs::write(repo.path().join("minds.yaml"), manifest).unwrap();
    repo
}

// ---------------------------------------------------------------------------
// 1. remove_project_success — directory + minds.yaml entry removed
// ---------------------------------------------------------------------------

#[test]
fn remove_project_success() {
    let repo = setup();
    let project_path = repo.path().join("alpha");
    assert!(project_path.exists(), "project should exist before removal");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "project", "remove", "alpha", "--yes"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    // Project directory removed
    assert!(!project_path.exists(), "project directory should be deleted");

    // minds.yaml no longer references alpha
    let manifest = std::fs::read_to_string(repo.path().join("minds.yaml")).unwrap();
    assert!(!manifest.contains("alpha"), "alpha should be removed from manifest");
    assert!(manifest.contains("beta"), "beta should still be in manifest");
}

// ---------------------------------------------------------------------------
// 2. remove_project_not_found — unknown project → usage error
// ---------------------------------------------------------------------------

#[test]
fn remove_project_not_found() {
    let repo = setup();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "project", "remove", "nonexistent", "--yes"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not found"), "stderr: {stderr}");
}

// ---------------------------------------------------------------------------
// 3. remove_project_json_envelope — JSON output with full envelope
// ---------------------------------------------------------------------------

#[test]
fn remove_project_json_envelope() {
    let repo = setup();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "project", "remove", "alpha", "--format", "json", "--yes"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    assert_eq!(v["data"]["kind"], "project");
    assert_eq!(v["data"]["identity"], "alpha");
    assert_eq!(v["data"]["removed"], true);
    assert_eq!(v["data"]["dry_run"], false);
}

// ---------------------------------------------------------------------------
// 4. remove_project_dry_run — dry run does not mutate
// ---------------------------------------------------------------------------

#[test]
fn remove_project_dry_run() {
    let repo = setup();
    let project_path = repo.path().join("alpha");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "project", "remove", "alpha", "--dry-run", "--yes"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    // Project still exists
    assert!(project_path.exists(), "project should still exist after dry run");

    // minds.yaml still references alpha
    let manifest = std::fs::read_to_string(repo.path().join("minds.yaml")).unwrap();
    assert!(manifest.contains("alpha"), "alpha should still be in manifest after dry run");
}

// ---------------------------------------------------------------------------
// 5. remove_project_rm_alias — `rm` alias works
// ---------------------------------------------------------------------------

#[test]
fn remove_project_rm_alias() {
    let repo = setup();
    let project_path = repo.path().join("alpha");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "project", "rm", "alpha", "--yes"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    assert!(!project_path.exists(), "project directory should be deleted via `rm` alias");
}
