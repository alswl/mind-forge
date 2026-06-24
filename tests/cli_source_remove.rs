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
  - name: my-feed
    type: rss
    url: https://example.com/feed.xml
    tags: []
    added_at: '2026-05-08T13:00:00Z'
    updated_at: '2026-05-08T13:00:00Z'
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
    std::fs::create_dir_all(project.join("sources/file")).unwrap();
    std::fs::write(project.join("sources/file/notes.md"), b"# notes").unwrap();
    seed_sources(&repo, "alpha");
    (repo, project)
}

// ---------------------------------------------------------------------------
// 1. remove_pdf_deletes_file_and_entry — file + entry removed
// ---------------------------------------------------------------------------

#[test]
fn remove_pdf_deletes_file_and_entry() {
    let (repo, project) = setup();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "remove", "paper", "--project", "alpha", "--yes"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    // File removed from disk
    assert!(!project.join("sources/pdf/paper.pdf").exists(), "file should be deleted");

    // Entry removed from index
    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(!index_content.contains("paper"), "entry should be removed, got: {index_content}");
}

// ---------------------------------------------------------------------------
// 2. remove_with_keep_file — --keep-file: entry removed, file stays
// ---------------------------------------------------------------------------

#[test]
fn remove_with_keep_file() {
    let (repo, project) = setup();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "remove",
            "paper",
            "--project",
            "alpha",
            "--keep-file",
            "--yes",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    // File still on disk
    assert!(project.join("sources/pdf/paper.pdf").exists(), "file should be kept");

    // Entry removed from index
    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(!index_content.contains("paper"), "entry should be removed");
}

// ---------------------------------------------------------------------------
// 3. remove_url_source_only_index — URL source: index only
// ---------------------------------------------------------------------------

#[test]
fn remove_url_source_only_index() {
    let (repo, project) = setup();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "remove",
            "research-blog",
            "--project",
            "alpha",
            "--yes",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    // Entry removed from index
    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(!index_content.contains("research-blog"), "entry should be removed");
}

// ---------------------------------------------------------------------------
// 4. remove_dirty_entry_succeeds — file already deleted externally
// ---------------------------------------------------------------------------

#[test]
fn remove_dirty_entry_succeeds() {
    let (repo, project) = setup();
    // Delete file externally
    std::fs::remove_file(project.join("sources/pdf/paper.pdf")).unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "remove", "paper", "--project", "alpha", "--yes"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    // Entry removed from index
    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(!index_content.contains("paper"), "entry should be removed");
}

// ---------------------------------------------------------------------------
// 5. remove_unknown_name — non-existent name → usage
// ---------------------------------------------------------------------------

#[test]
fn remove_unknown_name() {
    let (repo, _project) = setup();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "remove",
            "nonexistent",
            "--project",
            "alpha",
            "--yes",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not found"), "stderr: {stderr}");
}

// ---------------------------------------------------------------------------
// 6. remove_json_envelope — JSON mode with flatten Source + file_deleted
// ---------------------------------------------------------------------------

#[test]
fn remove_json_envelope() {
    let (repo, _project) = setup();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "remove",
            "paper",
            "--project",
            "alpha",
            "--output",
            "json",
            "--yes",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    assert_eq!(v["data"]["kind"], "source");
    assert_eq!(v["data"]["identity"], "paper");
    assert_eq!(v["data"]["removed"], true);
}
