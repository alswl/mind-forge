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
    added_at: '2026-05-01T10:00:00Z'
    updated_at: '2026-05-01T10:00:00Z'
  - name: research-blog
    type: web
    url: https://example.com/research
    tags: []
    added_at: '2026-05-01T11:00:00Z'
    updated_at: '2026-05-01T11:00:00Z'
  - name: my-feed
    type: rss
    url: https://example.com/feed.xml
    tags: []
    added_at: '2026-05-01T12:00:00Z'
    updated_at: '2026-05-01T12:00:00Z'
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
// 1. index_added_and_removed_summary — disk add + index remove → report correct
// ---------------------------------------------------------------------------

#[test]
fn index_added_and_removed_summary() {
    let (repo, project) = setup();
    // Add a new file on disk not in index
    std::fs::write(project.join("sources/pdf/new.pdf"), b"new pdf").unwrap();

    // Remove the indexed file from disk
    std::fs::remove_file(project.join("sources/pdf/paper.pdf")).unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "index", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("+ added:"), "should report added, got: {stdout}");
    assert!(stdout.contains("- removed:"), "should report removed, got: {stdout}");
}

// ---------------------------------------------------------------------------
// 2. index_keeps_url_sources_always — rss/web always kept
// ---------------------------------------------------------------------------

#[test]
fn index_keeps_url_sources_always() {
    let (repo, project) = setup();
    // Remove the indexed pdf from disk so it would be "removed"
    std::fs::remove_file(project.join("sources/pdf/paper.pdf")).unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "index", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    // Web and RSS sources should remain in the index
    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("research-blog"), "web source should be kept");
    assert!(index_content.contains("my-feed"), "rss source should be kept");
}

// ---------------------------------------------------------------------------
// 3. index_dry_run_no_writes — --dry-run doesn't modify index
// ---------------------------------------------------------------------------

#[test]
fn index_dry_run_no_writes() {
    let (repo, project) = setup();
    // Add a new file on disk
    std::fs::write(project.join("sources/pdf/new.pdf"), b"new pdf").unwrap();

    let before_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "index", "--project", "alpha", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("[dry-run]"), "dry-run output should have prefix, got: {stdout}");

    let after_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert_eq!(before_content, after_content, "index should not be modified");
}

// ---------------------------------------------------------------------------
// 4. index_missing_sources_dir — sources/ missing → usage hint
// ---------------------------------------------------------------------------

#[test]
fn index_missing_sources_dir() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    // Don't create sources/ dir

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "index", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("sources"), "stderr: {stderr}");
}

// ---------------------------------------------------------------------------
// 5. index_ignores_unknown_subdirs — sources/raw/ is ignored
// ---------------------------------------------------------------------------

#[test]
fn index_ignores_unknown_subdirs() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");

    // Create only sources/raw/ with a file (not sources/pdf/ or sources/file/)
    std::fs::create_dir_all(project.join("sources/raw")).unwrap();
    std::fs::write(project.join("sources/raw/data.bin"), b"data").unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "index", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    // Should have 0 added since raw/ is not scanned
    assert!(stdout.contains("kept: 0 entries"), "stdout: {stdout}");
}

// ---------------------------------------------------------------------------
// 6. index_ignores_hidden_files — .DS_Store / .gitkeep skipped
// ---------------------------------------------------------------------------

#[test]
fn index_ignores_hidden_files() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");

    std::fs::create_dir_all(project.join("sources/pdf")).unwrap();
    std::fs::write(project.join("sources/pdf/.gitkeep"), b"").unwrap();
    std::fs::write(project.join("sources/pdf/.DS_Store"), b"").unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "index", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.contains("gitkeep"), "should not index hidden files, got: {stdout}");
    assert!(stdout.contains("kept: 0 entries"), "stdout: {stdout}");
}

// ---------------------------------------------------------------------------
// 7. index_recursive_file_subdir — sources/file/sub/bar.md registered as file
// ---------------------------------------------------------------------------

#[test]
fn index_recursive_file_subdir() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");

    std::fs::create_dir_all(project.join("sources/file/sub")).unwrap();
    std::fs::write(project.join("sources/file/sub/bar.md"), b"# hello").unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "index", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("bar"), "stdout: {stdout}");

    // Index should have the entry with path including subdirectory
    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("sources/file/sub/bar.md"));
}

// ---------------------------------------------------------------------------
// 8. index_does_not_modify_kept_metadata — kept entries unchanged
// ---------------------------------------------------------------------------

#[test]
fn index_does_not_modify_kept_metadata() {
    let (repo, project) = setup();

    let before_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    // Extract the added_at for paper
    assert!(before_content.contains("added_at: '2026-05-01T10:00:00Z'"));

    // Run index — paper.pdf exists on disk so it's kept
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "index", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let after_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    // The original kept entry should still have the same added_at
    // (quotes may differ after serde_yaml round-trip)
    assert!(
        after_content.contains("2026-05-01T10:00:00Z"),
        "kept entry added_at should be preserved, got: {after_content}"
    );
}

// ---------------------------------------------------------------------------
// 9. index_json_envelope — JSON mode has dry_run field
// ---------------------------------------------------------------------------

#[test]
fn index_json_envelope() {
    let (repo, project) = setup();
    // Add a new file to have some non-trivial report
    std::fs::write(project.join("sources/pdf/new.pdf"), b"new pdf").unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "index", "--project", "alpha", "--format", "json"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    assert!(v["data"]["dry_run"].is_boolean());
    assert!(v["data"]["kept_count"].is_number());
    assert!(v["data"]["added"].is_array());
    assert!(v["data"]["removed"].is_array());
}

// ---------------------------------------------------------------------------
// 10. index_idempotent — consistent state: no changes on re-run
// ---------------------------------------------------------------------------

#[test]
fn index_idempotent() {
    let (repo, project) = setup();

    // First run
    let output1 = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "index", "--project", "alpha"])
        .output()
        .unwrap();
    assert!(output1.status.success());

    let content1 = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();

    // Second run — should be identical
    let output2 = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "index", "--project", "alpha"])
        .output()
        .unwrap();
    assert!(output2.status.success());

    let content2 = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();

    // The second run should result in 0 added, 0 removed
    let stdout2 = String::from_utf8(output2.stdout).unwrap();
    assert!(!stdout2.contains("+ added"), "second run should have no added, got: {stdout2}");
    assert!(!stdout2.contains("- removed"), "second run should have no removed, got: {stdout2}");

    // The index content should be the same (modulo timing differences)
    assert_eq!(content1, content2, "index should be idempotent");
}

// ---------------------------------------------------------------------------
// 11. index_preserves_other_sections — articles/assets not wiped
// ---------------------------------------------------------------------------

#[test]
fn index_preserves_other_sections() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");

    // Create index with sources + articles + assets sections
    let yaml = r#"schema_version: '1'
sources:
  - name: paper
    type: pdf
    path: sources/pdf/paper.pdf
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

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "index", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("articles:"), "articles section should be preserved");
    assert!(index_content.contains("my-post"), "article entry should be preserved");
    assert!(index_content.contains("assets:"), "assets section should be preserved");
    assert!(index_content.contains("logo"), "asset entry should be preserved");
}
