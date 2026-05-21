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
  - name: notes
    type: file
    path: sources/file/notes.md
    tags: []
    added_at: '2026-05-08T11:00:00Z'
    updated_at: '2026-05-08T11:00:00Z'
  - name: research-blog
    type: web
    url: https://example.com/research
    tags: []
    added_at: '2026-05-08T12:00:00Z'
    updated_at: '2026-05-08T12:00:00Z'
"#;
    common::write_index(repo, project_name, yaml);
}

fn setup() -> (TempDir, std::path::PathBuf) {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    std::fs::create_dir_all(project.join("sources/pdf")).unwrap();
    std::fs::write(project.join("sources/pdf/paper.pdf"), b"fake pdf").unwrap();
    std::fs::create_dir_all(project.join("sources/file")).unwrap();
    std::fs::write(project.join("sources/file/notes.md"), b"# notes").unwrap();
    seed_sources(&repo, "alpha");
    (repo, project)
}

// ---------------------------------------------------------------------------
// 1. rename_source_success — source renamed in index
// ---------------------------------------------------------------------------

#[test]
fn rename_source_success() {
    let (repo, _project) = setup();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "rename",
            "paper",
            "whitepaper",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let index_content = std::fs::read_to_string(repo.path().join("alpha").join("mind-index.yaml")).unwrap();
    assert!(!index_content.contains("name: paper"), "old name should be gone, got: {index_content}");
    assert!(index_content.contains("name: whitepaper"), "new name should be present, got: {index_content}");
}

// ---------------------------------------------------------------------------
// 2. rename_source_file_renamed — on-disk file is also renamed
// ---------------------------------------------------------------------------

#[test]
fn rename_source_file_renamed() {
    let (repo, project) = setup();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "rename",
            "notes",
            "meeting-notes",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    // Old file should not exist
    assert!(!project.join("sources/file/notes.md").exists(), "old file should be renamed");
    // New file should exist
    assert!(project.join("sources/file/meeting-notes.md").exists(), "new file should exist");
    // Index should reflect new name
    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("name: meeting-notes"), "new name should be in index");
}

// ---------------------------------------------------------------------------
// 3. rename_source_duplicate_refusal — renaming to existing source fails without --force
// ---------------------------------------------------------------------------

#[test]
fn rename_source_duplicate_refusal() {
    let (repo, _project) = setup();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "rename", "paper", "notes", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("already exists"), "stderr: {stderr}");
}

// ---------------------------------------------------------------------------
// 4. rename_source_dry_run — dry run does not mutate
// ---------------------------------------------------------------------------

#[test]
fn rename_source_dry_run() {
    let (repo, project) = setup();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "rename",
            "notes",
            "meeting-notes",
            "--dry-run",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    // File should still be at old location
    assert!(project.join("sources/file/notes.md").exists(), "old file should still exist after dry run");
    // Index should still have old name
    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("name: notes"), "old name should still be in index");
    assert!(!index_content.contains("name: meeting-notes"), "new name should not appear");
}

// ---------------------------------------------------------------------------
// 5. rename_source_not_found — unknown source → usage error
// ---------------------------------------------------------------------------

#[test]
fn rename_source_not_found() {
    let (repo, _project) = setup();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "rename",
            "nonexistent",
            "whatever",
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
// 6. rename_source_json_envelope — JSON output with full lifecycle envelope
// ---------------------------------------------------------------------------

#[test]
fn rename_source_json_envelope() {
    let (repo, _project) = setup();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "rename",
            "paper",
            "whitepaper",
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
    assert_eq!(v["data"]["verb"], "rename");
    assert_eq!(v["data"]["kind"], "source");
    assert_eq!(v["data"]["before"]["name"], "paper");
    assert_eq!(v["data"]["after"]["name"], "whitepaper");
    assert_eq!(v["data"]["force"], false);
    assert_eq!(v["data"]["dry_run"], false);
}
