use assert_cmd::Command;
use tempfile::TempDir;

mod common;

fn seed_sources(repo: &TempDir, project_name: &str) {
    let yaml = r#"schema_version: '1'
sources:
  - name: paper
    type: pdf
    path: sources/pdf/paper.pdf
    tags: []
    added_at: '2026-05-08T10:00:00Z'
    updated_at: '2026-05-08T10:00:00Z'
  - name: research-blog
    type: web
    url: https://example.com/research
    path: ~
    tags: []
    added_at: '2026-05-08T11:00:00Z'
    updated_at: '2026-05-08T11:00:00Z'
  - name: existing-name
    type: rss
    url: https://example.com/feed.xml
    path: ~
    tags: []
    added_at: '2026-05-08T12:00:00Z'
    updated_at: '2026-05-08T12:00:00Z'
"#;
    let yaml = yaml.replace("path: ~", "path:");
    common::write_index(repo, project_name, &yaml);
}

fn setup() -> (TempDir, std::path::PathBuf) {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    std::fs::create_dir_all(project.join("sources/pdf")).unwrap();
    std::fs::write(project.join("sources/pdf/paper.pdf"), b"fake pdf").unwrap();
    seed_sources(&repo, "alpha");
    (repo, project)
}

// ---------------------------------------------------------------------------
// 1. update_rename_only — --rename renames, other fields unchanged
// ---------------------------------------------------------------------------

#[test]
fn update_rename_only() {
    let (repo, _project) = setup();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "update",
            "paper",
            "--rename",
            "paper-v2",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout: {}, stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // Check index
    let project = repo.path().join("alpha");
    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("paper-v2"), "should have new name");
    assert!(!index_content.contains("\n  - name: paper\n"), "old name should be gone");
    // type and path unchanged
    assert!(index_content.contains("pdf"));
    assert!(index_content.contains("sources/pdf/paper.pdf"));
    // Disk file unchanged
    assert!(project.join("sources/pdf/paper.pdf").exists());
}

// ---------------------------------------------------------------------------
// 2. update_url_only — --url changes url, type/file unchanged
// ---------------------------------------------------------------------------

#[test]
fn update_url_only() {
    let (repo, _project) = setup();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "update",
            "paper",
            "--url",
            "https://example.com/paper-v2.pdf",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let index_content = std::fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert!(index_content.contains("https://example.com/paper-v2.pdf"));
    // type and path unchanged
    assert!(index_content.contains("pdf"));
    assert!(index_content.contains("sources/pdf/paper.pdf"));
}

// ---------------------------------------------------------------------------
// 3. update_combined_rename_and_url — both flags, single atomic write
// ---------------------------------------------------------------------------

#[test]
fn update_combined_rename_and_url() {
    let (repo, _project) = setup();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "update",
            "paper",
            "--rename",
            "paper-v2",
            "--url",
            "https://example.com/new.pdf",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let index_content = std::fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert!(index_content.contains("paper-v2"));
    assert!(index_content.contains("https://example.com/new.pdf"));
}

// ---------------------------------------------------------------------------
// 4. update_pdf_with_url_metadata — pdf entry gets --url metadata
// ---------------------------------------------------------------------------

#[test]
fn update_pdf_with_url_metadata() {
    let (repo, _project) = setup();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "update",
            "paper",
            "--url",
            "https://example.com/original.pdf",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let index_content = std::fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert!(index_content.contains("https://example.com/original.pdf"));
    assert!(index_content.contains("pdf"));
    assert!(index_content.contains("sources/pdf/paper.pdf"));
}

// ---------------------------------------------------------------------------
// 5. update_no_change_flags — no --rename or --url → usage
// ---------------------------------------------------------------------------

#[test]
fn update_no_change_flags() {
    let (repo, _project) = setup();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "update",
            "paper",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("nothing to update")
            || stderr.contains("--rename")
            || stderr.contains("--url"),
        "stderr: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// 6. update_rename_collision — --rename collides with existing name
// ---------------------------------------------------------------------------

#[test]
fn update_rename_collision() {
    let (repo, _project) = setup();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "update",
            "paper",
            "--rename",
            "research-blog",
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
// 7. update_unknown_name — non-existent name → usage
// ---------------------------------------------------------------------------

#[test]
fn update_unknown_name() {
    let (repo, _project) = setup();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "update",
            "nonexistent",
            "--rename",
            "something",
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
// 8. update_json_envelope — --format json returns full Source object
// ---------------------------------------------------------------------------

#[test]
fn update_json_envelope() {
    let (repo, _project) = setup();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "update",
            "paper",
            "--rename",
            "paper-v2",
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
    assert_eq!(v["data"]["name"], "paper-v2");
    assert_eq!(v["data"]["type"], "pdf");
}
