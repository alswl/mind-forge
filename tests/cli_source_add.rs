use assert_cmd::Command;
use tempfile::TempDir;

mod common;

/// Helper: create a Mind Repo + project named "alpha".
/// Returns (repo, source_dir, project_path, source_file_path).
fn setup() -> (TempDir, TempDir, std::path::PathBuf, std::path::PathBuf) {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    // Create a source file in a separate temp dir
    let source_dir = TempDir::new().unwrap();
    let source_file = source_dir.path().join("paper.pdf");
    std::fs::write(&source_file, b"fake pdf content").unwrap();
    (repo, source_dir, project, source_file)
}

// ---------------------------------------------------------------------------
// 1. add_file_copies_pdf — happy path copy + index entry + exit 0
// ---------------------------------------------------------------------------

#[test]
fn add_file_copies_pdf() {
    let (repo, _source_dir, project, source) = setup();
    let assert = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "add",
            source.to_str().unwrap(),
            "--project",
            "alpha",
        ])
        .assert();

    assert.success();

    // File was copied
    let dest = project.join("sources/pdf/paper.pdf");
    assert!(dest.exists(), "source file should exist in sources/pdf/");

    // Index has the entry
    let index_path = project.join("mind-index.yaml");
    let index_content = std::fs::read_to_string(&index_path).unwrap();
    assert!(index_content.contains("paper"));
    assert!(index_content.contains("pdf"));
}

// ---------------------------------------------------------------------------
// 2. add_file_with_explicit_name — --name custom overrides basename
// ---------------------------------------------------------------------------

#[test]
fn add_file_with_explicit_name() {
    let (repo, _source_dir, _project, source) = setup();
    let assert = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "add",
            source.to_str().unwrap(),
            "--project",
            "alpha",
            "--name",
            "my-paper",
        ])
        .assert();

    assert.success();

    // Check index entry name
    let project = repo.path().join("alpha");
    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("my-paper"));
    // File still uses original basename
    assert!(project.join("sources/pdf/paper.pdf").exists());
}

// ---------------------------------------------------------------------------
// 3. add_file_kind_inference — .md → file, .pdf → pdf
// ---------------------------------------------------------------------------

#[test]
fn add_file_kind_inference_md() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let source_dir = TempDir::new().unwrap();
    let source_file = source_dir.path().join("notes.md");
    std::fs::write(&source_file, b"# hello").unwrap();

    let assert = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "add",
            source_file.to_str().unwrap(),
            "--project",
            "alpha",
        ])
        .assert();

    assert.success();
    let project = repo.path().join("alpha");
    assert!(project.join("sources/file/notes.md").exists(), ".md files should go to sources/file/");
}

// ---------------------------------------------------------------------------
// 4. add_file_link_creates_symlink — --link creates symlink (unix only)
// ---------------------------------------------------------------------------

#[test]
#[cfg(unix)]
fn add_file_link_creates_symlink() {
    let (repo, _source_dir, project, source) = setup();
    let assert = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "add",
            source.to_str().unwrap(),
            "--project",
            "alpha",
            "--link",
        ])
        .assert();

    assert.success();
    let link = project.join("sources/pdf/paper.pdf");
    assert!(link.exists(), "symlink target should exist");
    assert_eq!(std::fs::read_link(&link).ok(), Some(source.canonicalize().unwrap()));
}

// ---------------------------------------------------------------------------
// 5. add_file_rejects_existing — same name second time → file-exists
// ---------------------------------------------------------------------------

#[test]
fn add_file_rejects_existing() {
    let (repo, _source_dir, _project, source) = setup();
    // First add succeeds
    Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "add",
            source.to_str().unwrap(),
            "--project",
            "alpha",
        ])
        .assert()
        .success();

    // Second add with same name (derived from basename) fails
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "add",
            source.to_str().unwrap(),
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success(), "should fail on duplicate");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("file-exists") || stderr.contains("already exists") || stderr.contains("refusing to overwrite"),
        "stderr: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// 6. add_file_force_overwrites — --force overwrites, updated_at refreshed
// ---------------------------------------------------------------------------

#[test]
fn add_file_force_overwrites() {
    let (repo, _source_dir, project, source) = setup();
    // First add
    Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "add",
            source.to_str().unwrap(),
            "--project",
            "alpha",
        ])
        .assert()
        .success();

    let index_path = project.join("mind-index.yaml");
    let first_content = std::fs::read_to_string(&index_path).unwrap();
    let added_at_match = first_content.lines().find(|l| l.contains("added_at")).unwrap_or("").to_string();

    // Second add with --force
    std::fs::write(&source, b"updated content").unwrap();
    let assert = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "add",
            source.to_str().unwrap(),
            "--project",
            "alpha",
            "--force",
        ])
        .assert();

    assert.success();

    // Verify added_at preserved
    let second_content = std::fs::read_to_string(&index_path).unwrap();
    assert!(second_content.contains(added_at_match.trim()), "added_at should be preserved");
}

// ---------------------------------------------------------------------------
// 7. add_outside_mind_repo — cwd not in a repo → not-in-mind-repo
// ---------------------------------------------------------------------------

#[test]
fn add_outside_mind_repo() {
    let outside = TempDir::new().unwrap();
    let source_file = outside.path().join("test.pdf");
    std::fs::write(&source_file, b"content").unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["source", "add", source_file.to_str().unwrap()])
        .current_dir(outside.path())
        .output()
        .unwrap();

    assert!(!output.status.success(), "should fail outside repo");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not in a mind repo"), "stderr: {stderr}");
}

// ---------------------------------------------------------------------------
// 8. add_without_project_context — in repo but no project → usage
// ---------------------------------------------------------------------------

#[test]
fn add_without_project_context() {
    let repo = common::setup_repo();
    // repo exists but no project created at cwd
    let source_dir = TempDir::new().unwrap();
    let source_file = source_dir.path().join("test.pdf");
    std::fs::write(&source_file, b"content").unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "add", source_file.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success(), "should fail without project context");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("could not detect") || stderr.contains("project"), "stderr: {stderr}");
}

// ---------------------------------------------------------------------------
// 9. add_path_invalid — nonexistent → usage
// ---------------------------------------------------------------------------

#[test]
fn add_path_invalid_nonexistent() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "add",
            "/tmp/nonexistent-file-12345.pdf",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success(), "should fail with nonexistent path");
}

// ---------------------------------------------------------------------------
// 10. add_self_reference_rejected — source inside sources/ → usage
// ---------------------------------------------------------------------------

#[test]
fn add_self_reference_rejected() {
    let (repo, _source_dir, project, source) = setup();
    // First add to create a source entry and file
    Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "add",
            source.to_str().unwrap(),
            "--project",
            "alpha",
        ])
        .assert()
        .success();

    // Now try to add the same file that's already inside sources/
    let already_inside = project.join("sources/pdf/paper.pdf");
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "add",
            already_inside.to_str().unwrap(),
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success(), "should reject self-reference");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("already inside") || stderr.contains("sources"), "stderr: {stderr}");
}

// =========================================================================
// URL class tests (US2)
// =========================================================================

// ---------------------------------------------------------------------------
// 11. add_url_web_happy — add https://... --name x → kind=web, no disk file
// ---------------------------------------------------------------------------

#[test]
fn add_url_web_happy() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");

    let assert = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "add",
            "https://example.com/research",
            "--project",
            "alpha",
            "--name",
            "research-blog",
        ])
        .assert();

    assert.success();

    // Index has the URL entry
    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("research-blog"));
    assert!(index_content.contains("web"));
    assert!(index_content.contains("https://example.com/research"));

    // No file created on disk
    assert!(!project.join("sources/web").exists());
}

// ---------------------------------------------------------------------------
// 12. add_url_requires_name — missing --name → usage
// ---------------------------------------------------------------------------

#[test]
fn add_url_requires_name() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "add",
            "https://example.com/no-name",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success(), "URL without --name should fail");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("URL sources require") || stderr.contains("--name"), "stderr: {stderr}");
}

// ---------------------------------------------------------------------------
// 13. add_url_rss_explicit — --type rss --name x → kind=rss
// ---------------------------------------------------------------------------

#[test]
fn add_url_rss_explicit() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");

    let assert = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "add",
            "https://example.com/feed.xml",
            "--project",
            "alpha",
            "--type",
            "rss",
            "--name",
            "my-feed",
        ])
        .assert();

    assert.success();

    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("rss"));
}

// ---------------------------------------------------------------------------
// 14. add_url_type_pdf_with_url_rejected — --type pdf + URL → usage
// ---------------------------------------------------------------------------

#[test]
fn add_url_type_pdf_with_url_rejected() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "add",
            "https://example.com/doc.pdf",
            "--project",
            "alpha",
            "--type",
            "pdf",
            "--name",
            "remote-pdf",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success(), "--type pdf + URL should fail");
}

// ---------------------------------------------------------------------------
// 15. add_url_type_file_with_url_rejected — --type file + URL → usage
// ---------------------------------------------------------------------------

#[test]
fn add_url_type_file_with_url_rejected() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "add",
            "https://example.com/notes",
            "--project",
            "alpha",
            "--type",
            "file",
            "--name",
            "remote-notes",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success(), "--type file + URL should fail");
}

// ---------------------------------------------------------------------------
// 16. add_url_invalid_scheme — non-http(s) → usage
// ---------------------------------------------------------------------------

#[test]
fn add_url_invalid_scheme() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");

    // also test missing host
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "add",
            "http://",
            "--project",
            "alpha",
            "--name",
            "empty-host",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success(), "http:// with empty host should fail");
}

// ---------------------------------------------------------------------------
// 17. add_url_force_replaces — same name + --force → updated_at refreshed
// ---------------------------------------------------------------------------

#[test]
fn add_url_force_replaces() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");

    // First add
    Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "add",
            "https://example.com/original",
            "--project",
            "alpha",
            "--name",
            "test-url",
        ])
        .assert()
        .success();

    // Second add with --force and different URL
    let assert = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "add",
            "https://example.com/updated",
            "--project",
            "alpha",
            "--name",
            "test-url",
            "--force",
        ])
        .assert();

    assert.success();

    let index_path = project.join("mind-index.yaml");
    let second_content = std::fs::read_to_string(&index_path).unwrap();
    // URL should be updated
    assert!(second_content.contains("https://example.com/updated"));
    // Entry should be present
    assert!(second_content.contains("test-url"));
}
