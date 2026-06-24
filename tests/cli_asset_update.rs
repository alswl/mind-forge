use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

mod common;

fn setup() -> (TempDir, std::path::PathBuf) {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    std::fs::create_dir_all(project.join("assets")).unwrap();

    // Write an asset file and create index entry
    std::fs::write(project.join("assets/cover.png"), b"original content").unwrap();
    let yaml = r#"schema_version: '1'
assets:
  - name: cover.png
    type: image
    path: assets/cover.png
    size: 16
    hash: "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2"
    tags: []
    added_at: '2026-05-07T19:30:00Z'
  - name: banner.jpg
    type: image
    path: assets/banner.jpg
    size: 24576
    hash: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    tags: []
    added_at: '2026-05-07T18:00:00Z'
"#;
    common::write_index(&repo, "alpha", yaml);
    (repo, project)
}

// Helpers to get hash from index
fn get_index_hash(project: &std::path::Path, name: &str) -> String {
    let content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    // Find hash for the asset by name — simple YAML parsing
    let entry_prefix = format!("  - name: {name}");
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].trim() == entry_prefix || lines[i].trim().starts_with(&format!("- name: {name}")) {
            // Look for hash in subsequent lines
            for line in lines.iter().skip(i + 1).take(10) {
                if let Some(h) = line.strip_prefix("    hash: \"") {
                    return h.trim_end_matches('"').to_string();
                }
                if let Some(h) = line.strip_prefix("    hash: ") {
                    return h.trim().trim_matches('"').to_string();
                }
            }
        }
        i += 1;
    }
    String::new()
}

// ---------------------------------------------------------------------------
// 1. single file size + hash refresh
// ---------------------------------------------------------------------------

#[test]
fn update_single_size_hash() {
    let (repo, project) = setup();
    // Modify the file
    std::fs::write(project.join("assets/cover.png"), b"modified content longer").unwrap();

    let assert = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "asset", "update", "assets/cover.png", "--project", "alpha"])
        .assert();

    assert.success().stdout(predicate::str::contains("updated"));
    // Hash should have changed
    let new_hash = get_index_hash(&project, "cover.png");
    assert_ne!(new_hash, "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2");
}

// ---------------------------------------------------------------------------
// 2. path resolution via basename only
// ---------------------------------------------------------------------------

#[test]
fn update_path_resolution_basename() {
    let (repo, project) = setup();
    std::fs::write(project.join("assets/cover.png"), b"modified content").unwrap();

    let assert = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "asset", "update", "cover.png", "--project", "alpha"])
        .assert();

    assert.success();
    let new_hash = get_index_hash(&project, "cover.png");
    assert_ne!(new_hash, "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2");
}

// ---------------------------------------------------------------------------
// 3. --all refresh
// ---------------------------------------------------------------------------

#[test]
fn update_all_refresh() {
    let (repo, project) = setup();
    // Also create the banner.jpg file
    std::fs::write(project.join("assets/banner.jpg"), b"banner content").unwrap();
    std::fs::write(project.join("assets/cover.png"), b"modified").unwrap();

    let assert = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "asset", "update", "--all", "--project", "alpha"])
        .assert();

    assert.success().stdout(predicate::str::contains("2 assets"));
}

// ---------------------------------------------------------------------------
// 4. <PATH> + --all mutually exclusive
// ---------------------------------------------------------------------------

#[test]
fn update_path_and_all_mutually_exclusive() {
    let (repo, _project) = setup();
    let assert = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "asset", "update", "cover.png", "--all", "--project", "alpha"])
        .assert();

    assert.code(predicate::eq(2));
}

// ---------------------------------------------------------------------------
// 5. no path and no --all
// ---------------------------------------------------------------------------

#[test]
fn update_no_path_no_all() {
    let (repo, _project) = setup();
    let assert = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "asset", "update", "--project", "alpha"])
        .assert();

    assert.code(predicate::eq(2));
}

// ---------------------------------------------------------------------------
// 6. missing file (single mode) → usage + index hint
// ---------------------------------------------------------------------------

#[test]
fn update_missing_file_single() {
    let (repo, _project) = setup();
    let assert = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "asset", "update", "nonexistent.png", "--project", "alpha"])
        .assert();

    assert.code(predicate::eq(2));
}

// ---------------------------------------------------------------------------
// 7. --all with missing file → result error + exit 0
// ---------------------------------------------------------------------------

#[test]
fn update_missing_file_all() {
    let (repo, _project) = setup();
    // Don't create banner.jpg on disk — only in index
    std::fs::write(repo.path().join("alpha/assets/cover.png"), b"content").unwrap();

    let assert = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "asset", "update", "--all", "--project", "alpha"])
        .assert();

    assert.success().stdout(predicate::str::contains("missing"));
}

// ---------------------------------------------------------------------------
// 8. broken symlink ≡ missing_file (Unix)
// ---------------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn update_symlink_broken() {
    let (repo, project) = setup();
    // Create a symlink entry without creating the target file
    std::os::unix::fs::symlink("/nonexistent/target", project.join("assets/link.png")).unwrap();
    // Add to index
    let yaml = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    let yaml = yaml.trim_end().to_string()
        + r#"
  - name: link.png
    type: image
    path: assets/link.png
    size: 0
    hash: "0000000000000000000000000000000000000000000000000000000000000000"
    tags: []
    added_at: '2026-05-07T19:30:00Z'
"#;
    std::fs::write(project.join("mind-index.yaml"), &yaml).unwrap();

    // Single mode
    let assert = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "asset", "update", "assets/link.png", "--project", "alpha"])
        .assert();

    assert.code(predicate::eq(2));
}

// ---------------------------------------------------------------------------
// 9. JSON envelope
// ---------------------------------------------------------------------------

#[test]
fn update_json_envelope() {
    let (repo, project) = setup();
    std::fs::write(project.join("assets/cover.png"), b"modified").unwrap();
    std::fs::write(project.join("assets/banner.jpg"), b"banner").unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "--output",
            "json",
            "asset",
            "update",
            "--all",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["status"], "ok");
    let data = &parsed["data"];
    let items = data["items"].as_array().unwrap();
    assert!(!items.is_empty());
    for item in items {
        assert!(item.get("path").is_some());
        assert!(item.get("changed").is_some());
        assert!(item.get("old_size").is_some());
        assert!(item.get("new_size").is_some());
        assert!(item.get("old_hash").is_some());
        assert!(item.get("new_hash").is_some());
    }
    assert!(data["summary"].is_object());
}
