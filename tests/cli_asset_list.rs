use assert_cmd::Command;
use tempfile::TempDir;

mod common;

/// Seed an index with assets
fn seed_assets(repo: &TempDir, project_name: &str) {
    let yaml = r#"schema_version: '1'
assets:
  - name: banner.jpg
    type: image
    path: assets/banner.jpg
    size: 24576
    hash: aabb1122
    tags: [hero]
    added_at: '2026-05-07T18:00:00Z'
  - name: cover.png
    type: image
    path: assets/cover.png
    size: 13580
    hash: ccdd3344
    tags: []
    added_at: '2026-05-07T19:30:00Z'
  - name: intro.mp4
    type: video
    path: assets/intro.mp4
    size: 1048576
    hash: eeff5566
    tags: [demo]
    added_at: '2026-05-07T20:00:00Z'
  - name: notes.pdf
    type: other
    path: assets/notes.pdf
    size: 4096
    hash: 77889900
    tags: []
    added_at: '2026-05-07T21:00:00Z'
"#;
    common::write_index(repo, project_name, yaml);
}

fn setup() -> (TempDir, std::path::PathBuf) {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    std::fs::create_dir_all(project.join("assets")).unwrap();
    seed_assets(&repo, "alpha");
    (repo, project)
}

// ---------------------------------------------------------------------------
// 1. table non-empty: ≥4 columns, alphabetical order
// ---------------------------------------------------------------------------

#[test]
fn list_table_non_empty() {
    let (repo, _project) = setup();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "asset", "list", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    // banner.jpg should come before cover.png
    let banner_pos = stdout.find("banner.jpg").unwrap();
    let cover_pos = stdout.find("cover.png").unwrap();
    assert!(banner_pos < cover_pos, "should be alphabetical");
    // Has header columns
    assert!(stdout.contains("NAME"));
    assert!(stdout.contains("TYPE"));
    assert!(stdout.contains("PATH"));
    assert!(stdout.contains("SIZE"));
}

// ---------------------------------------------------------------------------
// 2. empty message
// ---------------------------------------------------------------------------

#[test]
fn list_empty_message() {
    let (repo, _project) = setup();
    // Create a project with no assets
    common::create_project(&repo, "empty");
    std::fs::create_dir_all(repo.path().join("empty/assets")).unwrap();
    common::write_index(&repo, "empty", "schema_version: '1'\nassets: []\n");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "asset", "list", "--project", "empty"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("No assets found."));
}

// ---------------------------------------------------------------------------
// 3. JSON envelope
// ---------------------------------------------------------------------------

#[test]
fn list_json_envelope() {
    let (repo, _project) = setup();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "--format", "json", "asset", "list", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["status"], "ok");
    let data = parsed["data"].as_array().unwrap();
    assert_eq!(data.len(), 4);
    // Check fields
    for item in data {
        assert!(item.get("name").is_some());
        assert!(item.get("type").is_some());
        assert!(item.get("path").is_some());
        assert!(item.get("size").is_some());
        assert!(item.get("hash").is_some());
        assert!(item.get("tags").is_some());
        assert!(item.get("added_at").is_some());
    }
}

// ---------------------------------------------------------------------------
// 4. --filter substring (case-insensitive)
// ---------------------------------------------------------------------------

#[test]
fn list_filter_substring() {
    let (repo, _project) = setup();
    // "cover" matches name
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "asset", "list", "--project", "alpha", "--filter", "cover"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("cover.png"));
    assert!(!stdout.contains("banner.jpg"));

    // "HERO" matches tag (case-insensitive)
    let output2 = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "asset", "list", "--project", "alpha", "--filter", "HERO"])
        .output()
        .unwrap();

    assert!(output2.status.success());
    let stdout2 = String::from_utf8(output2.stdout).unwrap();
    assert!(stdout2.contains("banner.jpg"));
}

// ---------------------------------------------------------------------------
// 5. --type image
// ---------------------------------------------------------------------------

#[test]
fn list_type_image() {
    let (repo, _project) = setup();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "asset", "list", "--project", "alpha", "--type", "image"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("banner.jpg"));
    assert!(stdout.contains("cover.png"));
    assert!(!stdout.contains("intro.mp4"));
}

// ---------------------------------------------------------------------------
// 6. --type other
// ---------------------------------------------------------------------------

#[test]
fn list_type_other() {
    let (repo, _project) = setup();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "asset", "list", "--project", "alpha", "--type", "other"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("notes.pdf"));
    assert!(!stdout.contains("banner.jpg"));
}

// ---------------------------------------------------------------------------
// 7. --project cross-project
// ---------------------------------------------------------------------------

#[test]
fn list_cross_project() {
    let (repo, _project) = setup();
    // Create project "beta" with no assets
    common::create_project(&repo, "beta");
    std::fs::create_dir_all(repo.path().join("beta/assets")).unwrap();
    common::write_index(&repo, "beta", "schema_version: '1'\n");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "asset", "list", "--project", "beta"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("No assets found."));
}

// ---------------------------------------------------------------------------
// 8. outside mind repo
// ---------------------------------------------------------------------------

#[test]
fn list_outside_mind_repo() {
    let output = Command::cargo_bin("mf").unwrap().args(["asset", "list"]).output().unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not in a mind repo"));
}

// ---------------------------------------------------------------------------
// 9. missing index → empty list, exit 0
// ---------------------------------------------------------------------------

#[test]
fn list_missing_index() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    // No index file at all
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "asset", "list", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("No assets found."));
}

// ---------------------------------------------------------------------------
// 10. --type invalid → clap exit 2
// ---------------------------------------------------------------------------

#[test]
fn list_invalid_type() {
    let (repo, _project) = setup();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "asset", "list", "--project", "alpha", "--type", "bogus"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
}
