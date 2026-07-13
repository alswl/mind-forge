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

/// Seeds one bound prompt (`docs/my-post.md` <-> `prompts/my-post.md`), one
/// orphaned prompt (bound to a non-existent article), and two prompts that
/// both bind `docs/other.md` (duplicate).
fn setup_with_prompts() -> common::TempDir {
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
  - title: Other
    project: alpha
    type: blog
    article_path: docs/other.md
    status: draft
    created_at: '2026-07-01T00:00:00Z'
    updated_at: '2026-07-01T00:00:00Z'
prompts:
  - path: prompts/my-post.md
    article: docs/my-post.md
    mode: research
    updated_at: '2026-07-12T09:00:00Z'
  - path: prompts/widowed.md
    article: docs/does-not-exist.md
    updated_at: '2026-07-12T09:00:00Z'
  - path: prompts/other.md
    article: docs/other.md
    updated_at: '2026-07-12T09:00:00Z'
  - path: prompts/other-old.md
    article: docs/other.md
    updated_at: '2026-07-11T09:00:00Z'
"#;
    common::write_index(&repo, "alpha", index_yaml);
    repo
}

// ---------------------------------------------------------------------------
// list: binding status (bound / orphan / duplicate)
// ---------------------------------------------------------------------------

#[test]
fn list_shows_bound_orphan_and_duplicate_status() {
    let repo = setup_with_prompts();
    let (stdout, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "prompt", "list", "--project", "alpha"]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("prompts/my-post.md"), "{stdout}");
    assert!(stdout.contains("bound"), "{stdout}");
    assert!(stdout.contains("prompts/widowed.md"), "{stdout}");
    assert!(stdout.contains("orphan"), "{stdout}");
    assert!(stdout.contains("prompts/other.md"), "{stdout}");
    assert!(stdout.contains("duplicate"), "{stdout}");
}

#[test]
fn list_empty_friendly_message() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let (stdout, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "prompt", "list", "--project", "alpha"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("No prompts found."), "{stdout}");
}

// ---------------------------------------------------------------------------
// list: JSON envelope + identity round-trip
// ---------------------------------------------------------------------------

#[test]
fn list_json_envelope_and_identity_roundtrip() {
    let repo = setup_with_prompts();
    let (stdout, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "--output", "json", "prompt", "list", "--project", "alpha"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["status"], "ok");
    let prompts = parsed["data"]["prompts"].as_array().expect("prompts array");
    assert_eq!(prompts.len(), 4);

    let bound = prompts.iter().find(|p| p["path"] == "prompts/my-post.md").expect("bound prompt present");
    assert_eq!(bound["binding_status"], "bound");
    assert_eq!(bound["article"], "docs/my-post.md");
    assert_eq!(bound["mode"], "research");
    let identity = bound["identity"].as_str().expect("identity present").to_string();

    // Identity round-trips into `prompt show`.
    let (show_stdout, show_stderr, show_code) =
        run(&["--root", &repo.path().to_string_lossy(), "prompt", "show", &identity, "--project", "alpha"]);
    assert_eq!(show_code, 0, "stderr: {show_stderr}");
    assert!(show_stdout.contains("docs/my-post.md"), "{show_stdout}");
}

#[test]
fn list_json_prompts_is_array_when_empty() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let (stdout, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "--output", "json", "prompt", "list", "--project", "alpha"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["data"]["prompts"], serde_json::json!([]));
}

// ---------------------------------------------------------------------------
// list / show: read-only — must not mutate mind-index.yaml
// ---------------------------------------------------------------------------

#[test]
fn list_and_show_do_not_mutate_index() {
    let repo = setup_with_prompts();
    let index_path = repo.path().join("alpha/mind-index.yaml");
    let before = std::fs::read_to_string(&index_path).unwrap();

    let (_out, stderr, code) = run(&["--root", &repo.path().to_string_lossy(), "prompt", "list", "--project", "alpha"]);
    assert_eq!(code, 0, "stderr: {stderr}");

    let (_out, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "prompt", "show", "prompts/my-post.md", "--project", "alpha"]);
    assert_eq!(code, 0, "stderr: {stderr}");

    let after = std::fs::read_to_string(&index_path).unwrap();
    assert_eq!(before, after, "list/show must not rewrite mind-index.yaml");
}

// ---------------------------------------------------------------------------
// show: happy path, JSON, not-found
// ---------------------------------------------------------------------------

#[test]
fn show_happy_path() {
    let repo = setup_with_prompts();
    let (stdout, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "prompt", "show", "prompts/my-post.md", "--project", "alpha"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("docs/my-post.md"), "{stdout}");
    assert!(stdout.contains("research"), "{stdout}");
    assert!(stdout.contains("bound"), "{stdout}");
}

#[test]
fn show_json() {
    let repo = setup_with_prompts();
    let (stdout, stderr, code) = run(&[
        "--root",
        &repo.path().to_string_lossy(),
        "--output",
        "json",
        "prompt",
        "show",
        "prompts/my-post.md",
        "--project",
        "alpha",
    ]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["data"]["article"], "docs/my-post.md");
    assert_eq!(parsed["data"]["binding_status"], "bound");
}

#[test]
fn show_not_found_errors() {
    let repo = setup_with_prompts();
    let (_stdout, stderr, code) = run(&[
        "--root",
        &repo.path().to_string_lossy(),
        "prompt",
        "show",
        "prompts/does-not-exist.md",
        "--project",
        "alpha",
    ]);
    assert_eq!(code, 2, "stderr: {stderr}");
    assert!(stderr.contains("not found"), "{stderr}");
}

// ---------------------------------------------------------------------------
// missing repo / missing project
// ---------------------------------------------------------------------------

#[test]
fn list_outside_repo_errors() {
    let outside = tempfile::TempDir::new().unwrap();
    let output =
        Command::cargo_bin("mf").unwrap().args(["prompt", "list"]).current_dir(outside.path()).output().unwrap();
    assert!(!output.status.success());
}

/// Matches sibling commands (e.g. `source list`): an unresolved project name
/// falls through to an empty result rather than a hard error (FR-014,
/// "consistent with sibling entity commands").
#[test]
fn list_unknown_project_is_empty_not_error() {
    let repo = common::setup_repo();
    let (stdout, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "prompt", "list", "--project", "does-not-exist"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("No prompts found."), "{stdout}");
}

// ---------------------------------------------------------------------------
// index: reconcile (US2) — add / prune / keep, dry-run, idempotency,
// malformed-frontmatter tolerance
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
prompts:
  - path: prompts/stale.md
    article: docs/gone.md
    updated_at: '2026-07-01T00:00:00Z'
"#;
    common::write_index(&repo, "alpha", index_yaml);
    std::fs::create_dir_all(project.join("prompts")).unwrap();
    // `stale.md` is indexed but has no file on disk (must be pruned).
    std::fs::write(
        project.join("prompts/my-post.md"),
        "---\narticle: docs/my-post.md\nmode: research\n---\n\nWrite about the post.\n",
    )
    .unwrap();
    (repo, project)
}

#[test]
fn index_adds_present_and_prunes_stale() {
    let (repo, _project) = setup_for_index();
    let (stdout, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "--output", "json", "prompt", "index", "--project", "alpha"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["data"]["added"].as_array().unwrap().len(), 1);
    assert_eq!(parsed["data"]["removed"].as_array().unwrap().len(), 1);
    assert_eq!(parsed["data"]["kept_count"], 0);
    assert_eq!(parsed["data"]["dry_run"], false);

    let added = &parsed["data"]["added"][0];
    assert_eq!(added["path"], "prompts/my-post.md");
    assert_eq!(added["article"], "docs/my-post.md");
    assert_eq!(added["mode"], "research");

    let removed = &parsed["data"]["removed"][0];
    assert_eq!(removed["path"], "prompts/stale.md");

    // Reflected in a subsequent list.
    let (list_stdout, list_stderr, list_code) =
        run(&["--root", &repo.path().to_string_lossy(), "prompt", "list", "--project", "alpha"]);
    assert_eq!(list_code, 0, "stderr: {list_stderr}");
    assert!(list_stdout.contains("prompts/my-post.md"), "{list_stdout}");
    assert!(!list_stdout.contains("prompts/stale.md"), "{list_stdout}");
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
        "prompt",
        "index",
        "--project",
        "alpha",
        "--dry-run",
    ]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["data"]["dry_run"], true);
    assert_eq!(parsed["data"]["added"].as_array().unwrap().len(), 1);

    let after = std::fs::read_to_string(&index_path).unwrap();
    assert_eq!(before, after, "--dry-run must not write mind-index.yaml");
}

#[test]
fn index_is_idempotent_and_byte_stable() {
    let (repo, project) = setup_for_index();
    let (_stdout, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "prompt", "index", "--project", "alpha"]);
    assert_eq!(code, 0, "stderr: {stderr}");

    let index_path = project.join("mind-index.yaml");
    let after_first = std::fs::read_to_string(&index_path).unwrap();

    let (stdout, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "--output", "json", "prompt", "index", "--project", "alpha"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["data"]["added"].as_array().unwrap().len(), 0);
    assert_eq!(parsed["data"]["removed"].as_array().unwrap().len(), 0);
    assert_eq!(parsed["data"]["kept_count"], 1);

    let after_second = std::fs::read_to_string(&index_path).unwrap();
    assert_eq!(after_first, after_second, "second index run must be byte-stable");
}

#[test]
fn index_tolerates_malformed_frontmatter() {
    let (repo, project) = setup_for_index();
    // Malformed: no closing frontmatter delimiter.
    std::fs::write(project.join("prompts/broken.md"), "---\narticle: docs/my-post.md\n\nno closing delimiter\n")
        .unwrap();
    // No frontmatter at all.
    std::fs::write(project.join("prompts/no-frontmatter.md"), "Just some prose, no YAML block.\n").unwrap();

    let (stdout, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "--output", "json", "prompt", "index", "--project", "alpha"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    // All three files (my-post.md, broken.md, no-frontmatter.md) are
    // indexed without the run aborting.
    assert_eq!(parsed["data"]["added"].as_array().unwrap().len(), 3);

    let no_frontmatter =
        parsed["data"]["added"].as_array().unwrap().iter().find(|p| p["path"] == "prompts/no-frontmatter.md");
    assert!(no_frontmatter.is_some(), "no-frontmatter.md should still be projected: {stdout}");
    assert_eq!(no_frontmatter.unwrap()["article"], "", "unbound prompt has empty article, not a crash");

    let (list_stdout, list_stderr, list_code) =
        run(&["--root", &repo.path().to_string_lossy(), "prompt", "list", "--project", "alpha"]);
    assert_eq!(list_code, 0, "stderr: {list_stderr}");
    assert!(list_stdout.contains("prompts/broken.md"), "{list_stdout}");
    assert!(list_stdout.contains("prompts/no-frontmatter.md"), "{list_stdout}");
}
