use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

mod common;

/// Helper: create a Mind Repo + project named "alpha".
/// Returns (repo, source_dir, project_path, source_file_path).
fn setup() -> (TempDir, TempDir, std::path::PathBuf, std::path::PathBuf) {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    // Create assets/ directory
    std::fs::create_dir_all(project.join("assets")).unwrap();
    // Create a sample file in a separate temp dir (keep it alive!)
    let source_dir = TempDir::new().unwrap();
    let source_file = source_dir.path().join("sample.png");
    std::fs::write(&source_file, b"fake png content").unwrap();
    (repo, source_dir, project, source_file)
}

// ---------------------------------------------------------------------------
// 1. happy path copy + index entry + exit 0
// ---------------------------------------------------------------------------

#[test]
fn add_copies_file() {
    let (repo, _source_dir, project, source) = setup();
    let assert = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "asset", "add", source.to_str().unwrap(), "--project", "alpha"])
        .assert();

    assert.success();

    // File was copied
    let dest = project.join("assets/sample.png");
    assert!(dest.exists(), "asset file should exist");

    // Index has the entry
    let index_path = project.join("mind-index.yaml");
    assert!(index_path.exists());
    let index_content = std::fs::read_to_string(&index_path).unwrap();
    assert!(index_content.contains("sample.png"));
}

// ---------------------------------------------------------------------------
// 2. --tag multiple accumulation
// ---------------------------------------------------------------------------

#[test]
fn add_with_tags() {
    let (repo, _source_dir, _project, source) = setup();
    let assert = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "asset",
            "add",
            source.to_str().unwrap(),
            "--project",
            "alpha",
            "--tag",
            "screenshot",
            "--tag",
            "draft",
        ])
        .assert();

    assert.success();
}

// ---------------------------------------------------------------------------
// 3. --link creates a symlink (Unix only)
// ---------------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn add_link_creates_symlink() {
    let (repo, _source_dir, project, source) = setup();
    let assert = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "asset",
            "add",
            source.to_str().unwrap(),
            "--project",
            "alpha",
            "--link",
        ])
        .assert();

    assert.success();

    let link = project.join("assets/sample.png");
    assert!(link.exists(), "symlink target should be accessible");
    assert!(link.is_symlink(), "should be a symlink");
}

// ---------------------------------------------------------------------------
// 4. --copy --link mutually exclusive
// ---------------------------------------------------------------------------

#[test]
fn add_copy_link_mutually_exclusive() {
    let (repo, _source_dir, _project, source) = setup();
    let assert = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "asset",
            "add",
            source.to_str().unwrap(),
            "--project",
            "alpha",
            "--copy",
            "--link",
        ])
        .assert();

    // clap rejects mutually exclusive flags with exit code 2
    assert.code(predicate::eq(2));
}

// ---------------------------------------------------------------------------
// 5. file-exists rejection
// ---------------------------------------------------------------------------

#[test]
fn add_rejects_existing() {
    let (repo, _source_dir, _project, source) = setup();
    // First add succeeds
    Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "asset", "add", source.to_str().unwrap(), "--project", "alpha"])
        .assert()
        .success();

    // Second add fails with file-exists — error goes to stderr
    let assert = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "asset", "add", source.to_str().unwrap(), "--project", "alpha"])
        .assert();

    assert.code(predicate::eq(1)).stderr(predicate::str::contains("refusing to overwrite"));
}

// ---------------------------------------------------------------------------
// 6. --force overwrites
// ---------------------------------------------------------------------------

#[test]
fn add_force_overwrites() {
    let (repo, _source_dir, _project, source) = setup();
    // First add
    Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "asset", "add", source.to_str().unwrap(), "--project", "alpha"])
        .assert()
        .success();

    // Force overwrite
    let assert = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "asset",
            "add",
            source.to_str().unwrap(),
            "--project",
            "alpha",
            "--force",
        ])
        .assert();

    assert.success();
}

// ---------------------------------------------------------------------------
// 7. outside mind repo — error goes to stderr
// ---------------------------------------------------------------------------

#[test]
fn add_outside_mind_repo() {
    let outside = TempDir::new().unwrap();
    let source = outside.path().join("test.png");
    std::fs::write(&source, b"content").unwrap();

    let assert = Command::cargo_bin("mf").unwrap().args(["asset", "add", source.to_str().unwrap()]).assert();

    assert.code(predicate::eq(1)).stderr(predicate::str::contains("not in a mind repo"));
}

// ---------------------------------------------------------------------------
// 8. without project context (in repo but not in project)
// ---------------------------------------------------------------------------

#[test]
fn add_without_project_context() {
    let (repo, _source_dir, _project, source) = setup();
    let assert = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "asset", "add", source.to_str().unwrap()])
        .assert();

    assert.code(predicate::eq(2));
}

// ---------------------------------------------------------------------------
// 9. unknown project
// ---------------------------------------------------------------------------

#[test]
fn add_creates_project_dir_if_not_exists() {
    // When --root + --project are explicit, the path is valid even if the
    // project directory doesn't exist yet. The code creates it.
    let (repo, _source_dir, _project, source) = setup();
    let nonexistent_path = repo.path().join("nonexistent");
    assert!(!nonexistent_path.exists());
    Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "asset",
            "add",
            source.to_str().unwrap(),
            "--project",
            "nonexistent",
        ])
        .assert()
        .success();
    assert!(nonexistent_path.join("assets/sample.png").exists());
}

// ---------------------------------------------------------------------------
// 10. invalid source path
// ---------------------------------------------------------------------------

#[test]
fn add_invalid_source() {
    let (repo, _source_dir, _project, _source) = setup();
    let assert = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "asset",
            "add",
            "/nonexistent/path/file.png",
            "--project",
            "alpha",
        ])
        .assert();

    assert.code(predicate::eq(1)).stderr(predicate::str::contains("error:"));
}

// ---------------------------------------------------------------------------
// 11. self-reference: source inside assets/
// ---------------------------------------------------------------------------

#[test]
fn add_self_reference_rejected() {
    let (repo, _source_dir, project, _source) = setup();
    let inside = project.join("assets/self.png");
    std::fs::write(&inside, b"content").unwrap();

    let assert = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "asset", "add", inside.to_str().unwrap(), "--project", "alpha"])
        .assert();

    assert.code(predicate::eq(2));
}

// ---------------------------------------------------------------------------
// US3: Asset Layout tests
// ---------------------------------------------------------------------------

#[test]
fn add_uses_configured_asset_dir() {
    let repo = common::setup_repo();
    common::create_project(&repo, "custom-assets");
    common::write_mind_yaml(&repo, "custom-assets", "schema: '1'\npaths:\n  assets: media\n");

    let source_dir = TempDir::new().unwrap();
    let source_file = source_dir.path().join("cover.png");
    std::fs::write(&source_file, b"png content").unwrap();

    let _cwd = std::env::current_dir().unwrap();
    let project_path = repo.path().join("custom-assets");

    Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "asset",
            "add",
            source_file.to_str().unwrap(),
            "--project",
            "custom-assets",
        ])
        .assert()
        .success();

    // File should be in media/ not assets/
    assert!(!project_path.join("assets/cover.png").exists(), "should not use default assets/");
    assert!(project_path.join("media/cover.png").exists(), "should use configured media/ dir");

    // Index should use the configured prefix
    let index_path = project_path.join("mind-index.yaml");
    let index_content = std::fs::read_to_string(&index_path).unwrap();
    assert!(
        index_content.contains("media/cover.png"),
        "index entry should use configured asset dir prefix: {index_content}"
    );
}

#[test]
fn add_self_reference_rejected_with_configured_asset_dir() {
    let repo = common::setup_repo();
    common::create_project(&repo, "custom-assets");
    common::write_mind_yaml(&repo, "custom-assets", "schema: '1'\npaths:\n  assets: images\n");

    let source_dir = TempDir::new().unwrap();
    let source_file = source_dir.path().join("logo.png");
    std::fs::write(&source_file, b"logo").unwrap();

    Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "asset",
            "add",
            source_file.to_str().unwrap(),
            "--project",
            "custom-assets",
        ])
        .assert()
        .success();

    // Now try to add a file already inside images/
    let inside = repo.path().join("custom-assets/images/self.png");
    std::fs::write(&inside, b"content").unwrap();

    let assert = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "asset",
            "add",
            inside.to_str().unwrap(),
            "--project",
            "custom-assets",
        ])
        .assert();

    assert.code(predicate::eq(2));
}
