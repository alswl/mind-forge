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

// ---------------------------------------------------------------------------
// orphan_prompt / duplicate_binding (error severity, US3 T035)
// ---------------------------------------------------------------------------

fn setup_orphan_and_duplicate() -> common::TempDir {
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
  - path: prompts/widowed.md
    article: docs/does-not-exist.md
    updated_at: '2026-07-12T09:00:00Z'
  - path: prompts/my-post.md
    article: docs/my-post.md
    updated_at: '2026-07-12T09:00:00Z'
  - path: prompts/my-post-old.md
    article: docs/my-post.md
    updated_at: '2026-07-11T09:00:00Z'
"#;
    common::write_index(&repo, "alpha", index_yaml);
    std::fs::create_dir_all(project.join("prompts")).unwrap();
    std::fs::create_dir_all(project.join("thinking")).unwrap();
    std::fs::create_dir_all(project.join("docs")).unwrap();
    std::fs::create_dir_all(project.join("sources")).unwrap();
    std::fs::create_dir_all(project.join("assets")).unwrap();
    std::fs::write(project.join("docs/my-post.md"), "# My Post\n").unwrap();
    repo
}

#[test]
fn orphan_prompt_reported_as_error_and_fails_exit_code() {
    let repo = setup_orphan_and_duplicate();
    let (stdout, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "--output", "json", "project", "lint", "--project", "alpha"]);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let issues = parsed["data"]["issues"].as_array().expect("issues array");

    let orphan = issues.iter().find(|i| i["kind"] == "orphan_prompt").expect("orphan_prompt finding present");
    assert_eq!(orphan["severity"], "error");
    assert!(orphan["message"].as_str().unwrap().contains("prompts/widowed.md"), "{stdout}");

    assert_eq!(code, 1, "error-severity findings must fail lint: stderr={stderr}");
}

#[test]
fn duplicate_binding_reported_as_error() {
    let repo = setup_orphan_and_duplicate();
    let (stdout, _stderr, _code) =
        run(&["--root", &repo.path().to_string_lossy(), "--output", "json", "project", "lint", "--project", "alpha"]);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let issues = parsed["data"]["issues"].as_array().expect("issues array");

    let duplicate = issues.iter().find(|i| i["kind"] == "duplicate_binding").expect("duplicate_binding finding");
    assert_eq!(duplicate["severity"], "error");
    let msg = duplicate["message"].as_str().unwrap();
    assert!(msg.contains("docs/my-post.md"), "{msg}");
}

#[test]
fn rule_filter_restricts_to_orphan_prompt_only() {
    let repo = setup_orphan_and_duplicate();
    let (stdout, _stderr, _code) = run(&[
        "--root",
        &repo.path().to_string_lossy(),
        "--output",
        "json",
        "project",
        "lint",
        "--project",
        "alpha",
        "--rule",
        "orphan_prompt",
    ]);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let issues = parsed["data"]["issues"].as_array().expect("issues array");
    assert!(issues.iter().all(|i| i["kind"] == "orphan_prompt"), "{stdout}");
    assert!(!issues.is_empty(), "{stdout}");
}

// ---------------------------------------------------------------------------
// missing_thinking (warning severity, US3 T036)
// ---------------------------------------------------------------------------

#[test]
fn missing_thinking_reported_as_warning() {
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
  - path: prompts/my-post.md
    article: docs/my-post.md
    updated_at: '2026-07-12T09:00:00Z'
"#;
    common::write_index(&repo, "alpha", index_yaml);
    std::fs::create_dir_all(project.join("prompts")).unwrap();
    std::fs::create_dir_all(project.join("thinking")).unwrap();
    std::fs::create_dir_all(project.join("docs")).unwrap();
    std::fs::create_dir_all(project.join("sources")).unwrap();
    std::fs::create_dir_all(project.join("assets")).unwrap();
    std::fs::write(project.join("docs/my-post.md"), "# My Post\n").unwrap();
    // No thinking/my-post.md file — should trigger missing_thinking.

    let (stdout, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "--output", "json", "project", "lint", "--project", "alpha"]);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let issues = parsed["data"]["issues"].as_array().expect("issues array");
    let missing = issues.iter().find(|i| i["kind"] == "missing_thinking").expect("missing_thinking finding present");
    assert_eq!(missing["severity"], "warning");

    // A warning alone (no --max-warnings) must not fail the exit code.
    assert_eq!(code, 0, "stderr: {stderr}");
}

#[test]
fn missing_thinking_respects_max_warnings() {
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
  - path: prompts/my-post.md
    article: docs/my-post.md
    updated_at: '2026-07-12T09:00:00Z'
"#;
    common::write_index(&repo, "alpha", index_yaml);
    std::fs::create_dir_all(project.join("prompts")).unwrap();
    std::fs::create_dir_all(project.join("thinking")).unwrap();
    std::fs::create_dir_all(project.join("docs")).unwrap();
    std::fs::create_dir_all(project.join("sources")).unwrap();
    std::fs::create_dir_all(project.join("assets")).unwrap();
    std::fs::write(project.join("docs/my-post.md"), "# My Post\n").unwrap();

    let (_stdout, stderr, code) = run(&[
        "--root",
        &repo.path().to_string_lossy(),
        "project",
        "lint",
        "--project",
        "alpha",
        "--rule",
        "missing_thinking",
        "--max-warnings",
        "0",
    ]);
    assert_eq!(code, 1, "warnings exceeding --max-warnings must fail: stderr={stderr}");
}

// ---------------------------------------------------------------------------
// prompts/ and thinking/ are optional stores (T037, corrected from the
// original spec draft): unlike docs/sources/assets — guaranteed by
// `mf project new` via REQUIRED_PROJECT_DIRS — a project simply hasn't
// started the writing workflow yet if these are absent. `missing_directory`
// intentionally does NOT cover them; that would fail lint on every project
// that hasn't touched mf-plan/mf-write, which is the common case.
// ---------------------------------------------------------------------------

#[test]
fn absent_prompts_and_thinking_dirs_are_not_lint_findings() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    // Required docs/sources/assets exist; prompts/ and thinking/ do not —
    // this is the normal state for a project that only has articles so far.
    std::fs::create_dir_all(project.join("docs")).unwrap();
    std::fs::create_dir_all(project.join("sources")).unwrap();
    std::fs::create_dir_all(project.join("assets")).unwrap();

    let (stdout, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "--output", "json", "project", "lint", "--project", "alpha"]);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let issues = parsed["data"]["issues"].as_array().expect("issues array");

    let missing_dirs: Vec<&str> =
        issues.iter().filter(|i| i["kind"] == "missing_directory").map(|i| i["path"].as_str().unwrap_or("")).collect();
    assert!(!missing_dirs.iter().any(|p| p.contains("prompts")), "{stdout}");
    assert!(!missing_dirs.iter().any(|p| p.contains("thinking")), "{stdout}");
    assert_eq!(code, 0, "a project with no prompts/thinking activity yet must still pass lint: stderr={stderr}");
}

// ---------------------------------------------------------------------------
// clean project: no false positives
// ---------------------------------------------------------------------------

#[test]
fn clean_project_has_no_prompt_thinking_findings() {
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
  - path: prompts/my-post.md
    article: docs/my-post.md
    updated_at: '2026-07-12T09:00:00Z'
thinking:
  - path: thinking/my-post.md
    article: docs/my-post.md
    updated_at: '2026-07-12T09:00:00Z'
"#;
    common::write_index(&repo, "alpha", index_yaml);
    std::fs::create_dir_all(project.join("prompts")).unwrap();
    std::fs::create_dir_all(project.join("thinking")).unwrap();
    std::fs::create_dir_all(project.join("docs")).unwrap();
    std::fs::create_dir_all(project.join("sources")).unwrap();
    std::fs::create_dir_all(project.join("assets")).unwrap();
    std::fs::write(project.join("docs/my-post.md"), "# My Post\n").unwrap();

    let (stdout, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "--output", "json", "project", "lint", "--project", "alpha"]);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let issues = parsed["data"]["issues"].as_array().expect("issues array");
    let our_kinds = ["orphan_prompt", "duplicate_binding", "missing_thinking"];
    assert!(
        issues.iter().all(|i| !our_kinds.contains(&i["kind"].as_str().unwrap_or(""))),
        "clean project should have no prompt/thinking findings: {stdout}"
    );
    assert_eq!(code, 0, "stderr: {stderr}");
}
