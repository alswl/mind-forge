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

/// Like `run`, but forces TTY-mode rendering (headers, no truncation-hiding)
/// so header-row assertions are meaningful under the piped test harness.
fn run_tty(args: &[&str]) -> (String, String, i32) {
    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .env("MF_FORCE_TTY", "1")
        .args(args)
        .output()
        .expect("command runs");
    (
        String::from_utf8(output.stdout).expect("stdout utf8"),
        String::from_utf8(output.stderr).expect("stderr utf8"),
        output.status.code().unwrap_or_default(),
    )
}

/// Spec 065 Revision R1: prompt/thinking binding health is surfaced through
/// `mf article list`/`show`/`index`, not standalone `mf prompt`/`mf thinking`
/// command groups (those were removed before merge — see spec.md Revision R1).
///
/// Seeds one bound article (`docs/my-post.md` <-> `prompts/my-post.md` +
/// `thinking/my-post.md`), one orphaned prompt (bound to a non-existent
/// article), and two prompts that both bind `docs/other.md` (duplicate).
fn setup_with_bindings() -> common::TempDir {
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
  - title: Bare
    project: alpha
    type: blog
    article_path: docs/bare.md
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
thinking:
  - path: thinking/my-post.md
    article: docs/my-post.md
    updated_at: '2026-07-12T09:00:00Z'
"#;
    common::write_index(&repo, "alpha", index_yaml);
    repo
}

// ---------------------------------------------------------------------------
// T048: `mf article list` — PROMPT/THINKING indicator columns, read-only
// ---------------------------------------------------------------------------

#[test]
fn list_shows_prompt_mode_duplicate_and_thinking_presence() {
    let repo = setup_with_bindings();
    let (stdout, stderr, code) =
        run_tty(&["--root", &repo.path().to_string_lossy(), "article", "list", "--project", "alpha"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("PROMPT"), "{stdout}");
    assert!(stdout.contains("THINKING"), "{stdout}");
    assert!(stdout.contains("docs/my-post.md"), "{stdout}");
    assert!(stdout.contains("research"), "mode shown for bound prompt: {stdout}");
    assert!(stdout.contains("duplicate"), "duplicate binding shown: {stdout}");
    assert!(stdout.contains("docs/bare.md"), "{stdout}");
}

/// A bound prompt with no `mode:` frontmatter shows `bound` in the PROMPT
/// column, not `-` — `-` is reserved for "no prompt bound at all" so the two
/// states stay distinguishable.
#[test]
fn list_shows_bound_fallback_text_when_prompt_has_no_mode() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let index_yaml = r#"schema: '1'
articles:
  - title: No Mode
    project: alpha
    type: blog
    article_path: docs/no-mode.md
    status: draft
    created_at: '2026-07-01T00:00:00Z'
    updated_at: '2026-07-01T00:00:00Z'
prompts:
  - path: prompts/no-mode.md
    article: docs/no-mode.md
    updated_at: '2026-07-12T09:00:00Z'
"#;
    common::write_index(&repo, "alpha", index_yaml);
    let (stdout, stderr, code) =
        run_tty(&["--root", &repo.path().to_string_lossy(), "article", "list", "--project", "alpha"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let row = stdout.lines().find(|l| l.contains("docs/no-mode.md")).expect("row present");
    assert!(row.contains("bound"), "mode-less bound prompt should show 'bound', not '-': {row}");
}

#[test]
fn list_json_exposes_nullable_prompt_and_thinking_objects() {
    let repo = setup_with_bindings();
    let (stdout, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "--output", "json", "article", "list", "--project", "alpha"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let articles = parsed["data"]["articles"].as_array().expect("articles array");

    let bound = articles.iter().find(|a| a["article_path"] == "docs/my-post.md").expect("my-post present");
    assert_eq!(bound["prompt"]["path"], "prompts/my-post.md");
    assert_eq!(bound["prompt"]["binding_status"], "bound");
    assert_eq!(bound["prompt"]["mode"], "research");
    assert_eq!(bound["thinking"]["path"], "thinking/my-post.md");

    let duplicated = articles.iter().find(|a| a["article_path"] == "docs/other.md").expect("other present");
    assert_eq!(duplicated["prompt"]["binding_status"], "duplicate");
    let conflicts = duplicated["prompt"]["conflicts"].as_array().expect("conflicts array");
    assert_eq!(conflicts.len(), 2);

    let bare = articles.iter().find(|a| a["article_path"] == "docs/bare.md").expect("bare present");
    assert!(bare["prompt"].is_null(), "no prompt bound: {stdout}");
    assert!(bare["thinking"].is_null(), "no thinking file: {stdout}");
}

#[test]
fn list_does_not_mutate_index() {
    let repo = setup_with_bindings();
    let index_path = repo.path().join("alpha/mind-index.yaml");
    let before = std::fs::read_to_string(&index_path).unwrap();
    let (_out, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "article", "list", "--project", "alpha"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let after = std::fs::read_to_string(&index_path).unwrap();
    assert_eq!(before, after, "article list must not rewrite mind-index.yaml");
}

// ---------------------------------------------------------------------------
// T049: `mf article show` — prompt/thinking detail fields, conflicts on
// duplicate, read-only
// ---------------------------------------------------------------------------

#[test]
fn show_bound_prompt_and_thinking_fields() {
    let repo = setup_with_bindings();
    let (stdout, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "article", "show", "docs/my-post.md", "--project", "alpha"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("prompts/my-post.md"), "{stdout}");
    assert!(stdout.contains("research"), "{stdout}");
    assert!(stdout.contains("bound"), "{stdout}");
    assert!(stdout.contains("thinking/my-post.md"), "{stdout}");
}

#[test]
fn show_json_bound_prompt_and_thinking() {
    let repo = setup_with_bindings();
    let (stdout, stderr, code) = run(&[
        "--root",
        &repo.path().to_string_lossy(),
        "--output",
        "json",
        "article",
        "show",
        "docs/my-post.md",
        "--project",
        "alpha",
    ]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["data"]["prompt"]["path"], "prompts/my-post.md");
    assert_eq!(parsed["data"]["prompt"]["binding_status"], "bound");
    assert_eq!(parsed["data"]["prompt"]["mode"], "research");
    assert_eq!(parsed["data"]["thinking"]["path"], "thinking/my-post.md");
}

#[test]
fn show_duplicate_lists_every_conflicting_prompt_path() {
    let repo = setup_with_bindings();
    let (stdout, stderr, code) = run(&[
        "--root",
        &repo.path().to_string_lossy(),
        "--output",
        "json",
        "article",
        "show",
        "docs/other.md",
        "--project",
        "alpha",
    ]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["data"]["prompt"]["binding_status"], "duplicate");
    let conflicts: Vec<&str> =
        parsed["data"]["prompt"]["conflicts"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    assert_eq!(conflicts, vec!["prompts/other-old.md", "prompts/other.md"]);
}

#[test]
fn show_absent_prompt_and_thinking_is_null() {
    let repo = setup_with_bindings();
    let (stdout, stderr, code) = run(&[
        "--root",
        &repo.path().to_string_lossy(),
        "--output",
        "json",
        "article",
        "show",
        "docs/bare.md",
        "--project",
        "alpha",
    ]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert!(parsed["data"]["prompt"].is_null(), "{stdout}");
    assert!(parsed["data"]["thinking"].is_null(), "{stdout}");
}

#[test]
fn show_does_not_mutate_index() {
    let repo = setup_with_bindings();
    let index_path = repo.path().join("alpha/mind-index.yaml");
    let before = std::fs::read_to_string(&index_path).unwrap();
    let (_out, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "article", "show", "docs/my-post.md", "--project", "alpha"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let after = std::fs::read_to_string(&index_path).unwrap();
    assert_eq!(before, after, "article show must not rewrite mind-index.yaml");
}

// ---------------------------------------------------------------------------
// T050/T051/T052: `mf article index` — reconcile prompts + thinking in the
// same run; per-store additive envelope fields; dry-run; idempotency;
// malformed-frontmatter tolerance; legacy-scalar upgrade
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
thinking:
  - path: thinking/stale.md
    article: ''
    updated_at: '2026-07-01T00:00:00Z'
"#;
    common::write_index(&repo, "alpha", index_yaml);
    std::fs::create_dir_all(project.join("docs")).unwrap();
    std::fs::create_dir_all(project.join("prompts")).unwrap();
    std::fs::create_dir_all(project.join("thinking")).unwrap();
    // The article itself must exist on disk, or `article index` prunes it as stale.
    std::fs::write(project.join("docs/my-post.md"), "# My Post\n\nContent.\n").unwrap();
    // `stale.md` is indexed but has no file on disk (must be pruned) in both stores.
    std::fs::write(
        project.join("prompts/my-post.md"),
        "---\narticle: docs/my-post.md\nmode: research\n---\n\nWrite about the post.\n",
    )
    .unwrap();
    std::fs::write(project.join("thinking/my-post.md"), "## Notes\n\nWorking ledger.\n").unwrap();
    (repo, project)
}

#[test]
fn index_reconciles_prompts_and_thinking_in_one_run() {
    let (repo, _project) = setup_for_index();
    let (stdout, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "--output", "json", "article", "index", "--project", "alpha"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");

    let prompts = &parsed["data"]["prompts"];
    assert_eq!(prompts["added"].as_array().unwrap().len(), 1);
    assert_eq!(prompts["removed"].as_array().unwrap().len(), 1);
    assert_eq!(prompts["kept_count"], 0);
    assert_eq!(prompts["added"][0]["path"], "prompts/my-post.md");
    assert_eq!(prompts["added"][0]["article"], "docs/my-post.md");
    assert_eq!(prompts["added"][0]["mode"], "research");
    assert_eq!(prompts["removed"][0]["path"], "prompts/stale.md");

    let thinking = &parsed["data"]["thinking"];
    assert_eq!(thinking["added"].as_array().unwrap().len(), 1);
    assert_eq!(thinking["removed"].as_array().unwrap().len(), 1);
    assert_eq!(thinking["added"][0]["path"], "thinking/my-post.md");
    assert_eq!(thinking["added"][0]["article"], "docs/my-post.md");
    assert_eq!(thinking["removed"][0]["path"], "thinking/stale.md");

    // Reflected in a subsequent show.
    let (show_stdout, show_stderr, show_code) =
        run(&["--root", &repo.path().to_string_lossy(), "article", "show", "docs/my-post.md", "--project", "alpha"]);
    assert_eq!(show_code, 0, "stderr: {show_stderr}");
    assert!(show_stdout.contains("bound"), "{show_stdout}");
    assert!(show_stdout.contains("thinking/my-post.md"), "{show_stdout}");
}

#[test]
fn index_dry_run_covers_all_three_stores_and_does_not_write() {
    let (repo, project) = setup_for_index();
    let index_path = project.join("mind-index.yaml");
    let before = std::fs::read_to_string(&index_path).unwrap();

    let (stdout, stderr, code) = run(&[
        "--root",
        &repo.path().to_string_lossy(),
        "--output",
        "json",
        "article",
        "index",
        "--project",
        "alpha",
        "--dry-run",
    ]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["data"]["dry_run"], true);
    assert_eq!(parsed["data"]["prompts"]["added"].as_array().unwrap().len(), 1);
    assert_eq!(parsed["data"]["thinking"]["added"].as_array().unwrap().len(), 1);

    let after = std::fs::read_to_string(&index_path).unwrap();
    assert_eq!(before, after, "--dry-run must not write mind-index.yaml");
}

#[test]
fn index_is_idempotent_and_byte_stable() {
    let (repo, project) = setup_for_index();
    let (_stdout, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "article", "index", "--project", "alpha"]);
    assert_eq!(code, 0, "stderr: {stderr}");

    let index_path = project.join("mind-index.yaml");
    let after_first = std::fs::read_to_string(&index_path).unwrap();

    let (stdout, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "--output", "json", "article", "index", "--project", "alpha"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["data"]["prompts"]["added"].as_array().unwrap().len(), 0);
    assert_eq!(parsed["data"]["prompts"]["removed"].as_array().unwrap().len(), 0);
    assert_eq!(parsed["data"]["prompts"]["kept_count"], 1);
    assert_eq!(parsed["data"]["thinking"]["kept_count"], 1);

    let after_second = std::fs::read_to_string(&index_path).unwrap();
    assert_eq!(after_first, after_second, "second index run must be byte-stable");
}

#[test]
fn index_tolerates_malformed_prompt_frontmatter() {
    let (repo, project) = setup_for_index();
    // Malformed: no closing frontmatter delimiter.
    std::fs::write(project.join("prompts/broken.md"), "---\narticle: docs/my-post.md\n\nno closing delimiter\n")
        .unwrap();
    // No frontmatter at all.
    std::fs::write(project.join("prompts/no-frontmatter.md"), "Just some prose, no YAML block.\n").unwrap();

    let (stdout, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "--output", "json", "article", "index", "--project", "alpha"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let added = parsed["data"]["prompts"]["added"].as_array().unwrap();
    assert_eq!(added.len(), 3, "my-post.md, broken.md, no-frontmatter.md all indexed: {stdout}");

    let no_frontmatter = added.iter().find(|p| p["path"] == "prompts/no-frontmatter.md");
    assert!(no_frontmatter.is_some(), "no-frontmatter.md should still be projected: {stdout}");
    assert_eq!(no_frontmatter.unwrap()["article"], "", "unbound prompt has empty article, not a crash");
}

#[test]
fn index_tolerates_missing_prompts_and_thinking_directories() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    // Neither `prompts/` nor `thinking/` exists at all.
    let (stdout, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "--output", "json", "article", "index", "--project", "alpha"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["data"]["prompts"]["added"].as_array().unwrap().len(), 0);
    assert_eq!(parsed["data"]["thinking"]["added"].as_array().unwrap().len(), 0);
}

/// FR-011/SC-004: legacy opaque `prompts:`/`thinking:` scalar values must load
/// without error and be replaced by the typed projection on next reconcile.
#[test]
fn legacy_scalar_prompts_and_thinking_upgrade_without_error() {
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
prompts: active
thinking: active
"#;
    common::write_index(&repo, "alpha", index_yaml);
    std::fs::create_dir_all(project.join("prompts")).unwrap();

    // Loading via a read-only command must not error on the legacy scalar.
    let (_stdout, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "article", "list", "--project", "alpha"]);
    assert_eq!(code, 0, "legacy scalar prompts/thinking must not error: {stderr}");

    // Reconcile replaces the scalar with the typed projection.
    let (stdout, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "--output", "json", "article", "index", "--project", "alpha"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert!(parsed["data"]["prompts"].is_object(), "{stdout}");

    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(!index_content.contains("prompts: active"), "scalar value should be replaced:\n{index_content}");
}

// ---------------------------------------------------------------------------
// missing repo / missing project — consistent with sibling commands
// ---------------------------------------------------------------------------

#[test]
fn list_outside_repo_errors() {
    let outside = tempfile::TempDir::new().unwrap();
    let output =
        Command::cargo_bin("mf").unwrap().args(["article", "list"]).current_dir(outside.path()).output().unwrap();
    assert!(!output.status.success());
}
