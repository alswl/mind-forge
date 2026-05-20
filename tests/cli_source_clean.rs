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
// 1. clean_removes_dirty_entries — dirty pdf/file entries removed
// ---------------------------------------------------------------------------

#[test]
fn clean_removes_dirty_entries() {
    let (repo, project) = setup();
    // Delete the files externally
    std::fs::remove_file(project.join("sources/pdf/paper.pdf")).unwrap();
    std::fs::remove_file(project.join("sources/file/notes.md")).unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "clean", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("- removed:"), "should report removed, got: {stdout}");

    // Index should no longer have paper or notes
    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(!index_content.contains("paper"), "paper should be removed");
    assert!(!index_content.contains("notes"), "notes should be removed");
}

// ---------------------------------------------------------------------------
// 2. clean_does_not_add_new_files — files on disk NOT added to index
// ---------------------------------------------------------------------------

#[test]
fn clean_does_not_add_new_files() {
    let (repo, project) = setup();
    // Add a new file on disk that's not in the index
    std::fs::write(project.join("sources/pdf/new.pdf"), b"new pdf").unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "clean", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    // Should say "No dirty sources" since paper.pdf and notes.md still exist
    assert!(!stdout.contains("+ added"), "clean should not add new files, got: {stdout}");
}

// ---------------------------------------------------------------------------
// 3. clean_keeps_url_sources — rss/web always kept even if "dirty"
// ---------------------------------------------------------------------------

#[test]
fn clean_keeps_url_sources() {
    let (repo, project) = setup();
    // Delete paper to make a dirty entry
    std::fs::remove_file(project.join("sources/pdf/paper.pdf")).unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "clean", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    // URL sources should remain
    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("research-blog"), "web source kept");
    assert!(index_content.contains("my-feed"), "rss source kept");
}

// ---------------------------------------------------------------------------
// 4. clean_dry_run_no_writes — --dry-run doesn't modify index
// ---------------------------------------------------------------------------

#[test]
fn clean_dry_run_no_writes() {
    let (repo, project) = setup();
    std::fs::remove_file(project.join("sources/pdf/paper.pdf")).unwrap();

    let before_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "clean", "--project", "alpha", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("[dry-run]"), "should have prefix, got: {stdout}");

    let after_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert_eq!(before_content, after_content, "index should not be modified");
}

// ---------------------------------------------------------------------------
// 5. clean_no_dirty_message — no dirty entries → "No dirty sources."
// ---------------------------------------------------------------------------

#[test]
fn clean_no_dirty_message() {
    let (repo, _project) = setup();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "clean", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("No dirty sources."), "stdout: {stdout}");
}

// ---------------------------------------------------------------------------
// 6. clean_json_envelope — JSON data has removed, kept_count, dry_run, no added
// ---------------------------------------------------------------------------

#[test]
fn clean_json_envelope() {
    let (repo, project) = setup();
    std::fs::remove_file(project.join("sources/pdf/paper.pdf")).unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "clean", "--project", "alpha", "--format", "json"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    assert!(v["data"]["removed"].is_array());
    assert!(v["data"]["kept_count"].is_number());
    assert!(v["data"]["dry_run"].is_boolean());
    // added should be present (as empty array) since SourceIndexReport has it
}

// ---------------------------------------------------------------------------
// 7. clean_preserves_other_sections — articles/assets not wiped
// ---------------------------------------------------------------------------

#[test]
fn clean_preserves_other_sections() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");

    // Create index with sources + articles + assets sections; paper.pdf on disk
    let yaml = r#"schema_version: '1'
sources:
  - name: paper
    type: pdf
    path: sources/pdf/paper.pdf
    tags: []
    added_at: '2026-05-01T10:00:00Z'
    updated_at: '2026-05-01T10:00:00Z'
  - name: my-feed
    type: rss
    url: https://example.com/feed.xml
    tags: []
    added_at: '2026-05-01T10:00:00Z'
    updated_at: '2026-05-01T10:00:00Z'
articles:
  - title: my-post
    project: alpha
    type: blog
    article_path: docs/my-post.md
    status: draft
    created_at: '2026-05-01T10:00:00Z'
    updated_at: '2026-05-01T10:00:00Z'
assets:
  - name: logo
    type: image
    path: assets/logo.png
    size: 1024
    hash: abc
    tags: []
    added_at: '2026-05-01T10:00:00Z'
"#;
    common::write_index(&repo, "alpha", yaml);
    std::fs::create_dir_all(project.join("sources/pdf")).unwrap();
    std::fs::write(project.join("sources/pdf/paper.pdf"), b"fake pdf").unwrap();

    // Delete paper.pdf so clean removes it
    std::fs::remove_file(project.join("sources/pdf/paper.pdf")).unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "clean", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("articles:"), "articles section should be preserved");
    assert!(index_content.contains("my-post"), "article entry should be preserved");
    assert!(index_content.contains("assets:"), "assets section should be preserved");
    assert!(index_content.contains("logo"), "asset entry should be preserved");
    // paper is cleaned, my-feed (rss) should remain
    assert!(!index_content.contains("paper"), "dirty paper entry should be removed");
    assert!(index_content.contains("my-feed"), "rss source should be kept");
}
