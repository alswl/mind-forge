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
// 1. rename_asset_success — asset file and index entry renamed
// ---------------------------------------------------------------------------

#[test]
fn rename_asset_success() {
    let (repo, project) = setup();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "asset",
            "rename",
            "assets/images/diagram.png",
            "assets/images/architecture.png",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    // Old file should not exist
    assert!(!project.join("assets/images/diagram.png").exists(), "old file should be gone");
    // New file should exist
    assert!(project.join("assets/images/architecture.png").exists(), "new file should exist");
    // Index should reflect new path
    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(!index_content.contains("diagram.png"), "old path should be gone, got: {index_content}");
    assert!(index_content.contains("architecture.png"), "new path should be present, got: {index_content}");
}

// ---------------------------------------------------------------------------
// 2. rename_asset_duplicate_refusal — renaming to existing asset fails without --force
// ---------------------------------------------------------------------------

#[test]
fn rename_asset_duplicate_refusal() {
    let (repo, _project) = setup();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "asset",
            "rename",
            "assets/images/diagram.png",
            "assets/images/logo.png",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("already exists"), "stderr: {stderr}");
}

// ---------------------------------------------------------------------------
// 3. rename_asset_dry_run — dry run does not mutate
// ---------------------------------------------------------------------------

#[test]
fn rename_asset_dry_run() {
    let (repo, project) = setup();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "asset",
            "rename",
            "assets/images/diagram.png",
            "assets/images/architecture.png",
            "--dry-run",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    // Old file should still exist
    assert!(project.join("assets/images/diagram.png").exists(), "old file should still exist after dry run");
    // New file should not exist
    assert!(!project.join("assets/images/architecture.png").exists(), "new file should not exist after dry run");
    // Index should still have old path
    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("diagram.png"), "old path should still be in index");
    assert!(!index_content.contains("architecture.png"), "new path should not appear");
}

// ---------------------------------------------------------------------------
// 4. rename_asset_not_found — unknown asset → usage error
// ---------------------------------------------------------------------------

#[test]
fn rename_asset_not_found() {
    let (repo, _project) = setup();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "asset",
            "rename",
            "nonexistent.png",
            "whatever.png",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not found"), "stderr: {stderr}");
}

// ---------------------------------------------------------------------------
// 5. rename_asset_json_envelope — JSON output with full lifecycle envelope
// ---------------------------------------------------------------------------

#[test]
fn rename_asset_json_envelope() {
    let (repo, _project) = setup();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "asset",
            "rename",
            "assets/images/diagram.png",
            "assets/images/architecture.png",
            "--project",
            "alpha",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    assert_eq!(v["data"]["kind"], "asset");
    assert_eq!(v["data"]["old_identity"], "assets/images/diagram.png");
    assert_eq!(v["data"]["identity"], "assets/images/architecture.png");
    assert_eq!(v["data"]["dry_run"], false);
}
