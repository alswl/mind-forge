use assert_cmd::Command;
use tempfile::TempDir;

mod common;

fn seed_assets(repo: &TempDir, project_name: &str) {
    let yaml = r#"schema_version: '1'
assets:
  - name: diagram
    type: image
    path: assets/images/diagram.png
    size: 1024
    hash: abc123
    tags: []
    added_at: '2026-05-08T10:00:00Z'
  - name: logo
    type: image
    path: assets/images/logo.png
    size: 2048
    hash: def456
    tags: []
    added_at: '2026-05-08T11:00:00Z'
"#;
    common::write_index(repo, project_name, yaml);
}

fn setup() -> (TempDir, std::path::PathBuf) {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    std::fs::create_dir_all(project.join("assets/images")).unwrap();
    std::fs::write(project.join("assets/images/diagram.png"), b"fake png content").unwrap();
    std::fs::write(project.join("assets/images/logo.png"), b"fake png content").unwrap();
    seed_assets(&repo, "alpha");
    (repo, project)
}

// ---------------------------------------------------------------------------
// 1. remove_asset_success — file + entry removed
// ---------------------------------------------------------------------------

#[test]
fn remove_asset_success() {
    let (repo, project) = setup();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "asset",
            "remove",
            "assets/images/diagram.png",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    // File removed from disk
    assert!(!project.join("assets/images/diagram.png").exists(), "file should be deleted");

    // Entry removed from index
    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(!index_content.contains("diagram"), "entry should be removed");
}

// ---------------------------------------------------------------------------
// 2. remove_asset_not_found — unknown asset → usage error
// ---------------------------------------------------------------------------

#[test]
fn remove_asset_not_found() {
    let (repo, _project) = setup();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "asset", "remove", "nonexistent.pdf", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not found"), "stderr: {stderr}");
}

// ---------------------------------------------------------------------------
// 3. remove_asset_json_envelope — JSON output with full envelope
// ---------------------------------------------------------------------------

#[test]
fn remove_asset_json_envelope() {
    let (repo, _project) = setup();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "asset",
            "remove",
            "assets/images/diagram.png",
            "--project",
            "alpha",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    assert!(
        v["data"]["removed"].as_str().is_some_and(|s| s.contains("diagram")),
        "removed field should reference the asset"
    );
    assert_eq!(v["data"]["was_referenced"], false);
    assert_eq!(v["data"]["dry_run"], false);
}

// ---------------------------------------------------------------------------
// 4. remove_asset_dry_run — dry run does not mutate
// ---------------------------------------------------------------------------

#[test]
fn remove_asset_dry_run() {
    let (repo, project) = setup();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "asset",
            "remove",
            "assets/images/diagram.png",
            "--project",
            "alpha",
            "--dry-run",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    // File should still exist
    assert!(project.join("assets/images/diagram.png").exists(), "file should still exist after dry run");

    // Entry should still exist in index
    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("diagram"), "entry should still exist after dry run");
}

// ---------------------------------------------------------------------------
// 5. remove_asset_rm_alias — `rm` alias works
// ---------------------------------------------------------------------------

#[test]
fn remove_asset_rm_alias() {
    let (repo, project) = setup();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "asset",
            "rm",
            "assets/images/diagram.png",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    assert!(!project.join("assets/images/diagram.png").exists(), "file should be deleted via `rm` alias");
}
