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

#[test]
fn register_only_indexes_existing_in_tree_file_idempotently() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    let source_dir = project.join("sources/file");
    std::fs::create_dir_all(&source_dir).unwrap();
    let file = source_dir.join("synthetic.md");
    let bytes = b"synthetic source\n";
    std::fs::write(&file, bytes).unwrap();

    for _ in 0..2 {
        Command::cargo_bin("mf")
            .unwrap()
            .args([
                "--root",
                repo.path().to_str().unwrap(),
                "source",
                "new",
                file.to_str().unwrap(),
                "--project",
                "alpha",
                "--register-only",
            ])
            .assert()
            .success();
    }
    assert_eq!(std::fs::read(&file).unwrap(), bytes);
    let index = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert_eq!(index.matches("name: synthetic").count(), 1);
    assert!(index.contains("sources/file/synthetic.md"));
}

#[test]
fn register_only_dry_run_validates_but_does_not_write_index() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    std::fs::create_dir_all(project.join("sources")).unwrap();
    let file = project.join("sources/synthetic.md");
    std::fs::write(&file, b"synthetic\n").unwrap();
    Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "new",
            file.to_str().unwrap(),
            "--project",
            "alpha",
            "--register-only",
            "--dry-run",
        ])
        .assert()
        .success();
    assert!(!project.join("mind-index.yaml").exists());
}

#[test]
fn register_only_rejects_outside_file_and_link_mode() {
    let (repo, _source_dir, project, outside) = setup();
    let before = std::fs::read(&outside).unwrap();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "new",
            outside.to_str().unwrap(),
            "--project",
            "alpha",
            "--register-only",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(std::fs::read(&outside).unwrap(), before);
    assert!(!project.join("mind-index.yaml").exists());

    let inside = project.join("sources/synthetic.md");
    std::fs::create_dir_all(inside.parent().unwrap()).unwrap();
    std::fs::write(&inside, b"synthetic\n").unwrap();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "new",
            inside.to_str().unwrap(),
            "--project",
            "alpha",
            "--register-only",
            "--link",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
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
            "new",
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
            "new",
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
            "new",
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
            "new",
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
// 5. add_file_rejects_existing — same name second time → file_exists
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
            "new",
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
            "new",
            source.to_str().unwrap(),
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success(), "should fail on duplicate");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("file_exists") || stderr.contains("already exists") || stderr.contains("refusing to overwrite"),
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
            "new",
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
            "new",
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
// 7. add_outside_mind_repo — cwd not in a repo → not_in_mind_repo
// ---------------------------------------------------------------------------

#[test]
fn add_outside_mind_repo() {
    let outside = TempDir::new().unwrap();
    let source_file = outside.path().join("test.pdf");
    std::fs::write(&source_file, b"content").unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["source", "new", source_file.to_str().unwrap()])
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
        .args(["--root", repo.path().to_str().unwrap(), "source", "new", source_file.to_str().unwrap()])
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
            "new",
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
            "new",
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
            "new",
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
// URL class tests (Bug B)
//
// After Bug B every `mf source new <url>` fetches and stores a local file
// under `sources/<kind>/<name>.<ext>`.  The tests below use a loopback HTTP
// server so they do not depend on external network.
// =========================================================================
use std::io::Read;
use std::io::Write;
use std::net::TcpListener;

/// Start a tiny HTTP server on a random local port, serve `(status, body)` to
/// the next GET request, and return `http://127.0.0.1:<port>/<path>`.
fn start_http_mock(status: u16, body: &'static str, path: &str) -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let url = format!("http://{address}/{path}");
    let response = format!(
        "HTTP/1.1 {status} OK\r\nConnection: close\r\nContent-Length: {}\r\nContent-Type: text/html\r\n\r\n{body}",
        body.len()
    );
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let _ = stream.read(&mut [0_u8; 1024]);
        let _ = stream.write_all(response.as_bytes());
    });
    (url, server)
}

// ---------------------------------------------------------------------------
// 11. add_url_web_happy — add a web URL → local file + url+path registered
// ---------------------------------------------------------------------------

#[test]
fn add_url_web_happy() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");

    let (url, server) = start_http_mock(200, "<html>hello</html>", "research");
    let assert = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "new",
            &url,
            "--project",
            "alpha",
            "--name",
            "research-blog",
        ])
        .assert();

    assert.success();
    server.join().unwrap();

    // Local file created under sources/web/
    let local_file = project.join("sources/web/research-blog.html");
    assert!(local_file.exists(), "web source should be fetched and stored at {local_file:?}");
    let content = std::fs::read_to_string(&local_file).unwrap();
    assert_eq!(content, "<html>hello</html>");

    // Index has both url and path
    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("research-blog"));
    assert!(index_content.contains("web"));
    assert!(index_content.contains(&url));
    assert!(index_content.contains("sources/web/research-blog.html"));
}

// ---------------------------------------------------------------------------
// 12. add_url_requires_name — missing --name → usage (validated before fetch)
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
            "new",
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
// 13. add_url_rss_explicit — --file-kind rss → local file under sources/rss/
// ---------------------------------------------------------------------------

#[test]
fn add_url_rss_explicit() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");

    let (url, server) = start_http_mock(200, "<rss>feed</rss>", "feed.xml");
    let assert = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "new",
            &url,
            "--project",
            "alpha",
            "--file-kind",
            "rss",
            "--name",
            "my-feed",
        ])
        .assert();

    assert.success();
    server.join().unwrap();

    let local_file = project.join("sources/rss/my-feed.xml");
    assert!(local_file.exists(), "RSS source should be fetched and stored at {local_file:?}");

    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("rss"));
    assert!(index_content.contains("sources/rss/my-feed.xml"));
}

// ---------------------------------------------------------------------------
// 14. add_url_type_pdf_with_url_rejected — --file-kind pdf + URL → usage
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
            "new",
            "https://example.com/doc.pdf",
            "--project",
            "alpha",
            "--file-kind",
            "pdf",
            "--name",
            "remote-pdf",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success(), "--file-kind pdf + URL should fail");
}

// ---------------------------------------------------------------------------
// 15. add_url_type_file_with_url_rejected — --file-kind file + URL → usage
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
            "new",
            "https://example.com/notes",
            "--project",
            "alpha",
            "--file-kind",
            "file",
            "--name",
            "remote-notes",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success(), "--file-kind file + URL should fail");
}

// ---------------------------------------------------------------------------
// 16. add_url_invalid_scheme — http:// with empty host → usage
// ---------------------------------------------------------------------------

#[test]
fn add_url_invalid_scheme() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "new",
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
// 17. add_url_force_replaces — same name + --force → updated, old file cleaned
// ---------------------------------------------------------------------------

#[test]
fn add_url_force_replaces() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");

    // First add (fetches the "original" URL)
    let (url1, server1) = start_http_mock(200, "original content", "original");
    Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "new",
            &url1,
            "--project",
            "alpha",
            "--name",
            "test-url",
        ])
        .assert()
        .success();
    server1.join().unwrap();

    let first_file = project.join("sources/web/test-url.html");
    assert!(first_file.exists());
    assert_eq!(std::fs::read_to_string(&first_file).unwrap(), "original content");

    // Second add with --force and different URL
    let (url2, server2) = start_http_mock(200, "updated content", "updated");
    let assert = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "new",
            &url2,
            "--project",
            "alpha",
            "--name",
            "test-url",
            "--force",
        ])
        .assert();

    assert.success();
    server2.join().unwrap();

    let index_path = project.join("mind-index.yaml");
    let second_content = std::fs::read_to_string(&index_path).unwrap();
    // URL should be updated
    assert!(second_content.contains(&url2));
    // Entry should be present
    assert!(second_content.contains("test-url"));
    // Local file contains the new content
    assert_eq!(std::fs::read_to_string(&first_file).unwrap(), "updated content");
}
