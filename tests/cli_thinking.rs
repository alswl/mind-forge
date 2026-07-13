use assert_cmd::Command;

mod common;

fn run(args: &[&str]) -> (String, String, i32) {
    let output = Command::cargo_bin("mf").expect("binary exists").args(args).output().expect("command runs");
    (
        String::from_utf8(output.stdout).expect("stdout utf8"),
        String::from_utf8(output.stderr).expect("stderr utf8"),
        output.status.code().unwrap_or_default(),
    )
}

/// Seeds one bound thinking entry (`docs/my-post.md` <-> `thinking/my-post.md`
/// by key alignment) and one orphaned entry.
fn setup_with_thinking() -> common::TempDir {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let index_yaml = r#"schema: '1'
articles:
  - title: My Post
    project: alpha
    type: blog
    article_path: docs/my-post.md
    status: draft
    created_at: '2026-07-01T00:00:00Z'
    updated_at: '2026-07-01T00:00:00Z'
thinking:
  - path: thinking/my-post.md
    article: docs/my-post.md
    updated_at: '2026-07-12T09:00:00Z'
  - path: thinking/orphaned.md
    updated_at: '2026-07-12T09:00:00Z'
"#;
    common::write_index(&repo, "alpha", index_yaml);
    repo
}

#[test]
fn list_shows_bound_and_orphan_status() {
    let repo = setup_with_thinking();
    let (stdout, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "thinking", "list", "--project", "alpha"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("thinking/my-post.md"), "{stdout}");
    assert!(stdout.contains("bound"), "{stdout}");
    assert!(stdout.contains("thinking/orphaned.md"), "{stdout}");
    assert!(stdout.contains("orphan"), "{stdout}");
}

#[test]
fn list_empty_friendly_message() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let (stdout, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "thinking", "list", "--project", "alpha"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("No thinking"), "{stdout}");
}

#[test]
fn list_json_envelope_and_identity_roundtrip() {
    let repo = setup_with_thinking();
    let (stdout, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "--output", "json", "thinking", "list", "--project", "alpha"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["status"], "ok");
    let entries = parsed["data"]["thinking"].as_array().expect("thinking array");
    assert_eq!(entries.len(), 2);

    let bound = entries.iter().find(|t| t["path"] == "thinking/my-post.md").expect("bound entry present");
    assert_eq!(bound["binding_status"], "bound");
    let identity = bound["identity"].as_str().expect("identity present").to_string();

    let (show_stdout, show_stderr, show_code) =
        run(&["--root", &repo.path().to_string_lossy(), "thinking", "show", &identity, "--project", "alpha"]);
    assert_eq!(show_code, 0, "stderr: {show_stderr}");
    assert!(show_stdout.contains("docs/my-post.md"), "{show_stdout}");
}

#[test]
fn list_and_show_do_not_mutate_index() {
    let repo = setup_with_thinking();
    let index_path = repo.path().join("alpha/mind-index.yaml");
    let before = std::fs::read_to_string(&index_path).unwrap();

    let (_out, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "thinking", "list", "--project", "alpha"]);
    assert_eq!(code, 0, "stderr: {stderr}");

    let (_out, stderr, code) = run(&[
        "--root",
        &repo.path().to_string_lossy(),
        "thinking",
        "show",
        "thinking/my-post.md",
        "--project",
        "alpha",
    ]);
    assert_eq!(code, 0, "stderr: {stderr}");

    let after = std::fs::read_to_string(&index_path).unwrap();
    assert_eq!(before, after, "list/show must not rewrite mind-index.yaml");
}

#[test]
fn show_happy_path() {
    let repo = setup_with_thinking();
    let (stdout, stderr, code) = run(&[
        "--root",
        &repo.path().to_string_lossy(),
        "thinking",
        "show",
        "thinking/my-post.md",
        "--project",
        "alpha",
    ]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("docs/my-post.md"), "{stdout}");
    assert!(stdout.contains("bound"), "{stdout}");
}

#[test]
fn show_not_found_errors() {
    let repo = setup_with_thinking();
    let (_stdout, stderr, code) = run(&[
        "--root",
        &repo.path().to_string_lossy(),
        "thinking",
        "show",
        "thinking/does-not-exist.md",
        "--project",
        "alpha",
    ]);
    assert_eq!(code, 2, "stderr: {stderr}");
    assert!(stderr.contains("not found"), "{stderr}");
}

#[test]
fn list_outside_repo_errors() {
    let outside = tempfile::TempDir::new().unwrap();
    let output =
        Command::cargo_bin("mf").unwrap().args(["thinking", "list"]).current_dir(outside.path()).output().unwrap();
    assert!(!output.status.success());
}

// ---------------------------------------------------------------------------
// index: reconcile (US2) — add / prune / keep, dry-run, missing directory
// ---------------------------------------------------------------------------

fn setup_for_index() -> (common::TempDir, std::path::PathBuf) {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    let index_yaml = r#"schema: '1'
articles:
  - title: My Post
    project: alpha
    type: blog
    article_path: docs/my-post.md
    status: draft
    created_at: '2026-07-01T00:00:00Z'
    updated_at: '2026-07-01T00:00:00Z'
thinking:
  - path: thinking/stale.md
    article: ''
    updated_at: '2026-07-01T00:00:00Z'
"#;
    common::write_index(&repo, "alpha", index_yaml);
    std::fs::create_dir_all(project.join("thinking")).unwrap();
    // `stale.md` is indexed but has no file on disk (must be pruned).
    std::fs::write(project.join("thinking/my-post.md"), "## Notes\n\nWorking ledger.\n").unwrap();
    (repo, project)
}

#[test]
fn index_adds_present_and_prunes_stale() {
    let (repo, _project) = setup_for_index();
    let (stdout, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "--output", "json", "thinking", "index", "--project", "alpha"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["data"]["added"].as_array().unwrap().len(), 1);
    assert_eq!(parsed["data"]["removed"].as_array().unwrap().len(), 1);
    assert_eq!(parsed["data"]["kept_count"], 0);

    let added = &parsed["data"]["added"][0];
    assert_eq!(added["path"], "thinking/my-post.md");
    // Key-aligned to the existing `docs/my-post.md` article.
    assert_eq!(added["article"], "docs/my-post.md");

    let (list_stdout, list_stderr, list_code) =
        run(&["--root", &repo.path().to_string_lossy(), "thinking", "list", "--project", "alpha"]);
    assert_eq!(list_code, 0, "stderr: {list_stderr}");
    assert!(list_stdout.contains("thinking/my-post.md"), "{list_stdout}");
    assert!(!list_stdout.contains("thinking/stale.md"), "{list_stdout}");
}

#[test]
fn index_dry_run_does_not_write() {
    let (repo, project) = setup_for_index();
    let index_path = project.join("mind-index.yaml");
    let before = std::fs::read_to_string(&index_path).unwrap();

    let (stdout, stderr, code) = run(&[
        "--root",
        &repo.path().to_string_lossy(),
        "--output",
        "json",
        "thinking",
        "index",
        "--project",
        "alpha",
        "--dry-run",
    ]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["data"]["dry_run"], true);

    let after = std::fs::read_to_string(&index_path).unwrap();
    assert_eq!(before, after, "--dry-run must not write mind-index.yaml");
}

#[test]
fn index_tolerates_missing_thinking_directory() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    // No `thinking/` directory created at all.
    let (stdout, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "--output", "json", "thinking", "index", "--project", "alpha"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["data"]["added"].as_array().unwrap().len(), 0);
}
