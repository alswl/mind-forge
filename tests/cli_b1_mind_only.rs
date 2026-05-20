use assert_cmd::Command;
use tempfile::TempDir;

mod common;

// ---------------------------------------------------------------------------
// Setup helpers
// ---------------------------------------------------------------------------

fn setup_project(repo: &TempDir, name: &str) {
    common::create_project(repo, name);
    let project = repo.path().join(name);
    std::fs::create_dir_all(project.join("assets")).unwrap();
}

fn add_asset_entry(repo: &TempDir, project: &str, asset_name: &str, on_disk: bool) {
    let project_dir = repo.path().join(project);
    if on_disk {
        std::fs::write(project_dir.join("assets").join(asset_name), b"content").unwrap();
    }
    // Read existing index or create default
    let index_path = project_dir.join("mind-index.yaml");
    let mut yaml = std::fs::read_to_string(&index_path).unwrap_or_default();
    if yaml.trim().is_empty() {
        yaml = "schema_version: '1'\n".to_string();
    }
    // Append asset entry
    let entry = format!(
        r#"  - name: "{asset_name}"
    type: image
    path: "assets/{asset_name}"
    size: 7
    hash: "abc123"
    tags: []
    added_at: "2026-05-01T00:00:00Z"
"#
    );
    if yaml.contains("assets:") {
        let marker = if yaml.contains("\narticles:") {
            "\narticles:"
        } else if yaml.contains("\nsources:") {
            "\nsources:"
        } else {
            "\nterms:"
        };
        if yaml.contains(marker) {
            yaml = yaml.replace(marker, &format!("{entry}{marker}"));
        } else {
            yaml = format!("{yaml}{entry}");
        }
    } else {
        yaml = format!("{}assets:\n{}", yaml, entry);
    }
    std::fs::write(&index_path, &yaml).unwrap();
}

fn write_index_with_asset_and_article(
    project_dir: &std::path::Path,
    project: &str,
    asset_name: &str,
    article_name: &str,
) {
    let article_path = format!("docs/{article_name}.md");
    let yaml = format!(
        r#"schema_version: '1'
assets:
  - name: "{asset_name}"
    type: image
    path: "assets/{asset_name}"
    size: 7
    hash: "abc123"
    tags: []
    added_at: "2026-05-01T00:00:00Z"
articles:
  - title: "{article_name}"
    project: "{project}"
    type: blog
    article_path: "{article_path}"
    status: draft
    created_at: "2026-05-01T00:00:00Z"
    updated_at: "2026-05-01T00:00:00Z"
"#
    );
    std::fs::write(project_dir.join("mind-index.yaml"), &yaml).unwrap();
    // Write article body
    let doc_dir = project_dir.join("docs");
    std::fs::create_dir_all(&doc_dir).unwrap();
    std::fs::write(project_dir.join(&article_path), format!("uses {asset_name}")).unwrap();
}

// ---------------------------------------------------------------------------
// T064: mf asset clean - happy path (stale entries removed)
// ---------------------------------------------------------------------------

#[test]
fn asset_clean_removes_stale_entries() {
    let repo = common::setup_repo();
    setup_project(&repo, "test");
    // Add entry pointing to file that exists + one that doesn't
    add_asset_entry(&repo, "test", "exists.png", true);
    add_asset_entry(&repo, "test", "missing.png", false);

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "asset", "clean", "--project", "test"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Cleaned 1"), "should report 1 stale entry: {stdout}");
    assert!(stdout.contains("missing.png"), "should name missing file: {stdout}");
}

// ---------------------------------------------------------------------------
// T065: mf asset clean --dry-run (no writes)
// ---------------------------------------------------------------------------

#[test]
fn asset_clean_dry_run_does_not_write() {
    let repo = common::setup_repo();
    setup_project(&repo, "test");
    add_asset_entry(&repo, "test", "missing.png", false);
    let index_before = std::fs::read_to_string(repo.path().join("test/mind-index.yaml")).unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "asset", "clean", "--project", "test", "--dry-run"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[dry-run]"), "should indicate dry-run: {stdout}");
    // Verify index not changed
    let index_after = std::fs::read_to_string(repo.path().join("test/mind-index.yaml")).unwrap();
    assert_eq!(index_before, index_after, "dry-run should not modify index");
}

// ---------------------------------------------------------------------------
// T066: mf asset remove happy path
// ---------------------------------------------------------------------------

#[test]
fn asset_remove_deletes_file_and_entry() {
    let repo = common::setup_repo();
    setup_project(&repo, "test");
    add_asset_entry(&repo, "test", "logo.png", true);
    assert!(repo.path().join("test/assets/logo.png").exists(), "file should exist before removal");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "asset", "remove", "logo.png", "--project", "test"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("logo.png"), "should mention removed asset: {stdout}");
    assert!(!repo.path().join("test/assets/logo.png").exists(), "file should be deleted");
}

// ---------------------------------------------------------------------------
// T067: mf asset remove referenced without --force (error)
// ---------------------------------------------------------------------------

#[test]
fn asset_remove_referenced_errors_without_force() {
    let repo = common::setup_repo();
    setup_project(&repo, "test");
    let project_dir = repo.path().join("test");
    write_index_with_asset_and_article(&project_dir, "test", "logo.png", "welcome");
    std::fs::write(project_dir.join("assets/logo.png"), b"content").unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "asset", "remove", "logo.png", "--project", "test"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2), "should error: {}", String::from_utf8_lossy(&output.stderr));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("referenced"), "should mention referenced: {stderr}");
    // File should still exist
    assert!(repo.path().join("test/assets/logo.png").exists(), "file should remain");
}

// ---------------------------------------------------------------------------
// T068: mf asset remove referenced with --force
// ---------------------------------------------------------------------------

#[test]
fn asset_remove_referenced_with_force_succeeds() {
    let repo = common::setup_repo();
    setup_project(&repo, "test");
    let project_dir = repo.path().join("test");
    write_index_with_asset_and_article(&project_dir, "test", "logo.png", "welcome");
    std::fs::write(project_dir.join("assets/logo.png"), b"content").unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "asset", "remove", "logo.png", "--project", "test", "--force"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("logo.png"), "should mention removed asset: {stdout}");
    assert!(!repo.path().join("test/assets/logo.png").exists(), "file should be deleted despite reference");
}

// ---------------------------------------------------------------------------
// T069: mf project show happy path
// ---------------------------------------------------------------------------

#[test]
fn project_shows_details() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "project", "show", "my-project"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("my-project"), "should show name: {stdout}");
    assert!(stdout.contains("Articles:"), "should show article count: {stdout}");
}

// ---------------------------------------------------------------------------
// T070: mf project show nonexistent (error)
// ---------------------------------------------------------------------------

#[test]
fn project_show_nonexistent_errors() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "project", "show", "nonexistent"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2), "should error: {}", String::from_utf8_lossy(&output.stderr));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not found"), "should say not found: {stderr}");
}

// ---------------------------------------------------------------------------
// T071: mf project archive in git repo (happy path)
// ---------------------------------------------------------------------------

#[test]
fn project_archive_in_git_repo_succeeds() {
    let dir = TempDir::new().unwrap();
    // Init git repo
    std::process::Command::new("git").args(["init", "-q"]).current_dir(dir.path()).output().unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    std::process::Command::new("git").args(["config", "user.name", "Test"]).current_dir(dir.path()).output().unwrap();

    // Create minds.yaml and project
    std::fs::write(dir.path().join("minds.yaml"), "schema_version: '1'\nprojects_dir: '.'\nprojects: []\n").unwrap();
    let project_dir = dir.path().join("my-project");
    std::fs::create_dir_all(&project_dir).unwrap();
    std::fs::write(project_dir.join("mind.yaml"), "schema_version: '1'\n").unwrap();
    // Stage and commit
    std::process::Command::new("git").args(["add", "-A"]).current_dir(dir.path()).output().unwrap();
    std::process::Command::new("git").args(["commit", "-m", "initial"]).current_dir(dir.path()).output().unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", dir.path().to_str().unwrap(), "project", "archive", "my-project"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Archived"), "should confirm archive: {stdout}");
    assert!(!project_dir.exists(), "original project dir should be moved");
    assert!(dir.path().join("_archived/my-project").exists(), "archived dir should exist");
}

// ---------------------------------------------------------------------------
// T072: mf project archive without git repo (error)
// ---------------------------------------------------------------------------

#[test]
fn project_archive_non_git_repo_errors() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "project", "archive", "my-project"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2), "should error: {}", String::from_utf8_lossy(&output.stderr));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not a git repository"), "should mention git: {stderr}");
}

// ---------------------------------------------------------------------------
// T073: mf project import happy path
// ---------------------------------------------------------------------------

#[test]
fn project_import_creates_project_skeleton() {
    let repo = common::setup_repo();
    let import_dir = repo.path().join("to-import");
    std::fs::create_dir_all(import_dir.join("docs")).unwrap();
    std::fs::write(import_dir.join("docs/hello.md"), "# Hello").unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "project",
            "import",
            import_dir.to_str().unwrap(),
            "--type",
            "arch",
            "-y",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Imported"), "should confirm import: {stdout}");
    assert!(import_dir.join("mind.yaml").exists(), "mind.yaml should be created: {:?}", import_dir.join("mind.yaml"));
}

// ---------------------------------------------------------------------------
// T074: mf project import nonexistent directory (error)
// ---------------------------------------------------------------------------

#[test]
fn project_import_nonexistent_dir_errors() {
    let repo = common::setup_repo();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "project", "import", "/nonexistent/path", "-y"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2), "stderr: {}", String::from_utf8_lossy(&output.stderr));
}
