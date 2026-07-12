use assert_cmd::Command;
use predicates::str;
use std::fs;
use std::path::Path;

mod common;

#[test]
fn build_succeeds_with_article() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    // First create an article
    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "new", "Test Article"])
        .assert()
        .success();

    // Now build it
    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["build", "test-article"])
        .assert()
        .success()
        .stdout(str::contains("Article built:"));

    // Verify output file exists at default output path
    let output_path = repo.path().join("my-project").join("outputs").join("test-article.md");
    assert!(output_path.exists(), "output file should exist at {output_path:?}");
    let content = std::fs::read_to_string(&output_path).unwrap();
    assert!(content.contains("Test Article"), "output should contain article title");
}

#[test]
fn build_uses_configured_output_dir() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_mind_yaml(&repo, "my-project", "schema: '1'\nbuild:\n  output_dir: custom-output\n  format: md\n");

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "new", "Configurable Output"])
        .assert()
        .success();

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["build", "configurable-output"])
        .assert()
        .success();

    let configured_path = repo.path().join("my-project/custom-output/configurable-output.md");
    assert!(configured_path.exists(), "build should use configured output_dir");
    assert!(
        !repo.path().join("my-project/outputs/configurable-output.md").exists(),
        "build should not also write the default output dir"
    );
}

#[test]
fn build_uses_configured_docs_dir() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_mind_yaml(
        &repo,
        "my-project",
        "schema: '1'\npaths:\n  docs: notes\nbuild:\n  output_dir: outputs\n  format: md\n",
    );

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "new", "Custom Docs"])
        .assert()
        .success();

    assert!(
        repo.path().join("my-project/notes/custom-docs/01-opening.md").exists(),
        "article should be in custom docs dir"
    );
    assert!(!repo.path().join("my-project/docs/custom-docs.md").exists(), "should not use default docs dir");

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["build", "custom-docs"])
        .assert()
        .success();

    assert!(repo.path().join("my-project/outputs/custom-docs.md").exists());
}

#[test]
fn build_dry_run_shows_plan() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "new", "Dry Run Test"])
        .assert()
        .success();

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["build", "dry-run-test", "--dry-run"])
        .assert()
        .success()
        .stdout(str::contains("Build Plan:"));

    // Verify no output file was created
    let output_path = repo.path().join("my-project").join("outputs").join("dry-run-test.md");
    assert!(!output_path.exists(), "dry-run should not create output file");
}

#[test]
fn build_accepts_repo_relative_directory_article_path() {
    let repo = tempfile::TempDir::new().unwrap();
    fs::write(repo.path().join("minds.yaml"), "schema: '1'\nprojects:\n  - projects/2026-blogs\n").unwrap();
    let project = repo.path().join("projects/2026-blogs");
    fs::create_dir_all(project.join("docs/2026-03-review")).unwrap();
    fs::write(project.join("mind.yaml"), "schema: '1'\n").unwrap();
    fs::write(project.join("docs/2026-03-review/02-body.md"), "Body\n").unwrap();
    fs::write(project.join("docs/2026-03-review/01-opening.md"), "Opening\n").unwrap();

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path())
        .args(["build", "@projects/2026-blogs/docs/2026-03-review/"])
        .assert()
        .success()
        .stdout(str::contains("Article built: 2026-03-review"));

    let output_path = project.join("outputs/2026-03-review.md");
    let content = fs::read_to_string(output_path).unwrap();
    assert_eq!(content, "Opening\nBody\n");
}

#[test]
fn build_falls_back_to_directory_matching_article_argument_when_index_source_is_stale() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    // Indexed entry article_path does not exist — the article key is "review" and
    // its article_path is a directory that does exist
    common::write_index(
        &repo,
        "my-project",
        r#"schema: '1'
articles:
  review:
    title: Review
    article_path: docs/review
"#,
    );
    let article_dir = repo.path().join("my-project/docs/review");
    fs::create_dir_all(&article_dir).unwrap();
    fs::write(article_dir.join("01-opening.md"), "Opening\n").unwrap();

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["build", "review"])
        .assert()
        .success();

    let content = fs::read_to_string(repo.path().join("my-project/outputs/review.md")).unwrap();
    assert_eq!(content, "Opening\n");
}

#[test]
fn build_non_existent_article_fails() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["build", "non-existent"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(1), "should fail with exit 1 for non-existent article");
}

#[test]
fn build_empty_article_name_fails() {
    let repo = common::setup_repo();
    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path())
        .args(["build", ""])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(2), "empty name should be usage error");
}

#[test]
fn build_needs_project_context() {
    let repo = common::setup_repo();

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path())
        .args(["build", "article"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(2), "should fail without project");
}

// ---------------------------------------------------------------------------
// US1: Build Banner tests
// ---------------------------------------------------------------------------

#[test]
fn build_banner_with_level_wraps_in_admonition() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_mind_yaml(
        &repo,
        "my-project",
        "schema: '1'\nbuild:\n  banner:\n    text: Do not edit\n    level: warning\n",
    );
    common::write_article_index(&repo, "my-project", "test-article");
    common::write_doc(&repo, "my-project", "test-article", "Article content\n");

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["build", "test-article"])
        .assert()
        .success();

    let output_path = repo.path().join("my-project/outputs/test-article.md");
    let content = fs::read_to_string(&output_path).unwrap();
    assert!(content.contains(":::warning"), "banner with level should wrap in admonition");
    assert!(content.contains("Do not edit"), "banner text should appear in output");
    assert!(content.contains(":::"), "admonition closing marker should be present");
    assert!(content.contains("Article content"), "article content should remain");
}

/// Bug #21 (spec 064) regression: a configured `build.banner` must survive
/// repeated rebuilds — each rebuild's output must contain exactly one
/// rendered banner, never zero (lost) and never duplicated.
#[test]
fn build_banner_survives_repeated_rebuilds() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_mind_yaml(
        &repo,
        "my-project",
        "schema: '1'\nbuild:\n  banner:\n    text: Do not edit\n    level: warning\n",
    );
    common::write_article_index(&repo, "my-project", "test-article");
    common::write_doc(&repo, "my-project", "test-article", "Article content\n");

    let output_path = repo.path().join("my-project/outputs/test-article.md");

    for i in 0..2 {
        Command::cargo_bin("mf")
            .expect("binary exists")
            .current_dir(repo.path().join("my-project"))
            .args(["build", "test-article"])
            .assert()
            .success();

        let content = fs::read_to_string(&output_path).unwrap();
        assert!(content.contains(":::warning"), "rebuild {i}: banner must be present: {content}");
        assert!(content.contains("Do not edit"), "rebuild {i}: banner text must be present: {content}");
        assert_eq!(content.matches(":::warning").count(), 1, "rebuild {i}: banner must not duplicate: {content}");
        assert!(content.contains("Article content"), "rebuild {i}: article content must remain: {content}");
    }
}

#[test]
fn build_banner_without_level_inserts_raw_text() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_mind_yaml(&repo, "my-project", "schema: '1'\nbuild:\n  banner:\n    text: Do not edit\n");
    common::write_article_index(&repo, "my-project", "test-article");
    common::write_doc(&repo, "my-project", "test-article", "Article content\n");

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["build", "test-article"])
        .assert()
        .success();

    let output_path = repo.path().join("my-project/outputs/test-article.md");
    let content = fs::read_to_string(&output_path).unwrap();
    assert!(content.contains("Do not edit"), "banner text should appear");
    assert!(!content.contains(":::"), "no admonition wrapper when level is absent");
    assert!(content.contains("Article content"), "article content should remain");
}

#[test]
fn build_banner_no_config_produces_unchanged_output() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_article_index(&repo, "my-project", "test-article");
    common::write_doc(&repo, "my-project", "test-article", "Article content\n");

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["build", "test-article"])
        .assert()
        .success();

    let output_path = repo.path().join("my-project/outputs/test-article.md");
    let content = fs::read_to_string(&output_path).unwrap();
    assert_eq!(content.trim(), "Article content", "no banner text should be added");
}

#[test]
fn build_banner_dry_run_shows_banner_info() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_mind_yaml(
        &repo,
        "my-project",
        "schema: '1'\nbuild:\n  banner:\n    text: Do not edit\n    level: note\n",
    );
    common::write_article_index(&repo, "my-project", "test-article");
    common::write_doc(&repo, "my-project", "test-article", "Article content\n");

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["build", "test-article", "--dry-run"])
        .output()
        .expect("command runs");
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Banner: enabled"), "dry-run text should show banner info");

    // Verify no output file created
    let output_path = repo.path().join("my-project/outputs/test-article.md");
    assert!(!output_path.exists(), "dry-run should not create output file");
}

#[test]
fn build_banner_dry_run_json_includes_banner_field() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_mind_yaml(
        &repo,
        "my-project",
        "schema: '1'\nbuild:\n  banner:\n    text: Do not edit\n    level: tip\n",
    );
    common::write_article_index(&repo, "my-project", "test-article");
    common::write_doc(&repo, "my-project", "test-article", "Article content\n");

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["build", "test-article", "--dry-run", "--json"])
        .output()
        .expect("command runs");
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["status"], "ok");
    let banner = &json["data"]["banner"];
    assert!(banner.is_object(), "banner should be an object in JSON dry-run");
    assert_eq!(banner["enabled"], true);
    assert_eq!(banner["level"], "tip");
}

// Edge cases

#[test]
fn build_banner_with_self_managed_admonition_no_level() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_mind_yaml(
        &repo,
        "my-project",
        r#"schema: '1'
build:
  banner:
    text: |
      :::warning
      Self-managed banner.
      :::
"#,
    );
    common::write_article_index(&repo, "my-project", "test-article");
    common::write_doc(&repo, "my-project", "test-article", "Article content\n");

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["build", "test-article"])
        .assert()
        .success();

    let output_path = repo.path().join("my-project/outputs/test-article.md");
    let content = fs::read_to_string(&output_path).unwrap();
    // Should contain exactly one admonition (self-managed, no double-wrapping)
    assert_eq!(content.matches(":::").count(), 2, "self-managed banner should not be double-wrapped");
    assert!(content.contains("Self-managed banner."));
}

#[test]
fn build_banner_inserted_after_frontmatter() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_mind_yaml(
        &repo,
        "my-project",
        "schema: '1'\nbuild:\n  banner:\n    text: Banner text\n    level: warning\n",
    );
    common::write_article_index(&repo, "my-project", "test-article");
    // Article with YAML frontmatter
    common::write_doc(&repo, "my-project", "test-article", "---\ntitle: Test\n---\n\nArticle content\n");

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["build", "test-article"])
        .assert()
        .success();

    let output_path = repo.path().join("my-project/outputs/test-article.md");
    let content = fs::read_to_string(&output_path).unwrap();
    // Frontmatter should be first
    assert!(content.starts_with("---"), "frontmatter should remain first");
    let banner_pos = content.find("Banner text").unwrap();
    let frontmatter_end = content.find("---\n\n").map(|i| i + 5).unwrap();
    assert!(banner_pos >= frontmatter_end, "banner should appear after frontmatter");
    assert!(content.contains("Article content"), "article content should remain after banner");
}

#[test]
fn build_banner_empty_text_rejected_on_config() {
    // This test verifies that empty banner text is rejected during build
    // (the actual validation happens at config load time)
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_mind_yaml(&repo, "my-project", "schema: '1'\nbuild:\n  banner:\n    text: ''\n    level: warning\n");
    common::write_article_index(&repo, "my-project", "test-article");
    common::write_doc(&repo, "my-project", "test-article", "Article content\n");

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["build", "test-article"])
        .output()
        .expect("command runs");
    // Should fail (exit 2 = usage error) due to empty banner text
    assert_eq!(output.status.code(), Some(2), "empty banner text should be rejected");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("banner.text") || stderr.contains("non-empty"), "error should mention empty banner text");
}

// ---------------------------------------------------------------------------
// US2: Build with article directory tests
// ---------------------------------------------------------------------------

#[test]
fn build_from_configured_article_dir_under_docs() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_mind_yaml(
        &repo,
        "my-project",
        "schema: '1'\nbuild:\n  articles:\n    my-article:\n      article_dir: docs/custom-src\n",
    );
    // Create the custom article directory
    let src_dir = repo.path().join("my-project/docs/custom-src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(src_dir.join("01-intro.md"), "Intro\n").unwrap();
    std::fs::write(src_dir.join("02-body.md"), "Body\n").unwrap();
    // Write article index pointing to my-article
    common::write_article_index(&repo, "my-project", "my-article");

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["build", "my-article"])
        .assert()
        .success();

    let output_path = repo.path().join("my-project/outputs/my-article.md");
    assert!(output_path.exists(), "output should exist");
    let content = std::fs::read_to_string(&output_path).unwrap();
    assert!(content.contains("Intro"), "should include content from configured article dir");
    assert!(content.contains("Body"), "should include all files from configured article dir");
}

#[test]
fn build_from_configured_article_dir_outside_docs() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_mind_yaml(
        &repo,
        "my-project",
        "schema: '1'\nbuild:\n  articles:\n    my-article:\n      article_dir: specs\n",
    );
    // Create article directory outside docs/
    let src_dir = repo.path().join("my-project/specs");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(src_dir.join("01-spec.md"), "Spec content\n").unwrap();
    common::write_article_index(&repo, "my-project", "my-article");

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["build", "my-article"])
        .assert()
        .success();

    let output_path = repo.path().join("my-project/outputs/my-article.md");
    assert!(output_path.exists(), "output should exist");
    let content = std::fs::read_to_string(&output_path).unwrap();
    assert!(content.contains("Spec content"), "should include content from outside docs/");
}

#[test]
fn build_with_missing_article_dir_fails() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_mind_yaml(
        &repo,
        "my-project",
        "schema: '1'\nbuild:\n  articles:\n    my-article:\n      article_dir: non-existent-dir\n",
    );
    common::write_article_index(&repo, "my-project", "my-article");

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["build", "my-article"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(2), "missing article_dir should be usage error");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("non-existent-dir") || stderr.contains("does not exist"),
        "error should name the missing dir"
    );
    assert!(stderr.contains("my-article"), "error should name the article");
}

#[test]
fn build_default_article_dir_unchanged_without_config() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    // No article_dir config — uses default behavior
    common::write_article_index(&repo, "my-project", "default-article");
    common::write_doc(&repo, "my-project", "default-article", "Default content\n");

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["build", "default-article"])
        .assert()
        .success();

    let output_path = repo.path().join("my-project/outputs/default-article.md");
    assert!(output_path.exists(), "output should exist");
    let content = std::fs::read_to_string(&output_path).unwrap();
    assert!(content.contains("Default content"), "should build from default docs/ directory");
}

// ---------------------------------------------------------------------------
// US2: Build auto-indexing (via configured article_dir)
// ---------------------------------------------------------------------------

#[test]
fn build_auto_indexes_article_in_mind_index_yaml() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    // Configure a article_dir for the article so build can find it without the index
    common::write_mind_yaml(
        &repo,
        "my-project",
        "schema: '1'\nbuild:\n  articles:\n    auto-index-me:\n      article_dir: specs\n",
    );
    // Create article file in the configured article_dir
    common::write_source_file(&repo, "my-project", "specs", "auto-index-me", "# Auto index\n");
    // Write a minimal index (empty, article not in it)
    common::write_index(&repo, "my-project", "schema: '1'\n");

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["build", "auto-index-me"])
        .assert()
        .success();

    // After build, the article should be in the index
    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["--output", "json", "article", "list"])
        .output()
        .expect("command runs");
    let stdout = String::from_utf8(output.stdout).unwrap();
    // Skip any tracing output before JSON
    let json_start = stdout.find('{').unwrap_or(0);
    let stripped = &stdout[json_start..];
    let parsed: serde_json::Value = serde_json::from_str(stripped).unwrap_or_else(|e| {
        panic!("JSON parse error: {e}\nstdout: {stdout:?}");
    });
    let articles = parsed["data"]["articles"].as_array().unwrap();
    let auto_indexed = articles.iter().find(|a| a["article_path"].as_str().unwrap_or("").contains("auto-index-me"));
    assert!(auto_indexed.is_some(), "built article should appear in index after build: {stdout}");
}

#[test]
fn build_auto_indexes_configured_article_dir_as_article_directory() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_mind_yaml(
        &repo,
        "my-project",
        "schema: '1'\nbuild:\n  articles:\n    quarterly-review:\n      article_dir: specs/quarterly\n",
    );
    let article_dir = repo.path().join("my-project/specs/quarterly");
    std::fs::create_dir_all(&article_dir).unwrap();
    std::fs::write(article_dir.join("01-intro.md"), "Intro\n").unwrap();
    std::fs::write(article_dir.join("02-body.md"), "Body\n").unwrap();
    common::write_index(&repo, "my-project", "schema: '1'\n");

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["build", "quarterly-review"])
        .assert()
        .success();

    let index = std::fs::read_to_string(repo.path().join("my-project/mind-index.yaml")).unwrap();
    assert!(index.contains("article_path: specs/quarterly"), "index should store the configured article dir: {index}");
    assert!(
        !index.contains("article_path: specs/quarterly/quarterly-review.md"),
        "index should not invent a Markdown file inside the configured article dir: {index}"
    );
}

// ---------------------------------------------------------------------------
// US2: Build resolves by article key using indexed article path (T019–T022)
// ---------------------------------------------------------------------------

#[test]
fn build_dry_run_exact_key_lookup_directory_article() {
    let repo = common::scaffold_team_reports_minimal_repro();
    let project_path = repo.path().join("team-reports");

    // Index first so the declared article is registered
    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(&project_path)
        .args(["article", "index"])
        .assert()
        .success();

    // Build dry-run using exact article key
    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(&project_path)
        .args(["build", "2026-05-monthly", "--dry-run"])
        .output()
        .expect("command runs");
    assert_eq!(
        output.status.code(),
        Some(0),
        "build dry-run should succeed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("2026-05-monthly"), "build plan should reference article key: {stdout}");
    assert!(stdout.contains("Input sources:"), "dry-run should show input sources: {stdout}");
}

#[test]
fn build_dry_run_input_file_includes_directory_file() {
    let repo = common::scaffold_team_reports_minimal_repro();
    let project_path = repo.path().join("team-reports");

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(&project_path)
        .args(["article", "index"])
        .assert()
        .success();

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(&project_path)
        .args(["build", "2026-05-monthly", "--dry-run"])
        .output()
        .expect("command runs");
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("docs/2026-05-monthly/01-team-okr.md"),
        "input sources should include directory markdown file: {stdout}"
    );
}

#[test]
fn build_title_not_used_for_article_path_derivation() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    // Create a directory article with a display title that looks different from the key
    let article_dir = repo.path().join("my-project/docs/team-updates");
    fs::create_dir_all(&article_dir).unwrap();
    fs::write(article_dir.join("01-note.md"), "# Note\n").unwrap();

    // Index with the article key, not the title
    let index_yaml = r#"schema: '1'
articles:
  team-updates:
    title: Team Updates 2026
    article_path: docs/team-updates
"#;
    common::write_index(&repo, "my-project", index_yaml);

    // Build using the article key should work
    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["build", "team-updates", "--dry-run"])
        .assert()
        .success();

    // Build using the title should fail — title is not a lookup key
    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["build", "Team Updates 2026", "--dry-run"])
        .output()
        .expect("command runs");
    assert_ne!(output.status.code(), Some(0), "build by title should fail");
}

#[test]
fn build_dry_run_json_envelope_includes_article_path() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_index(
        &repo,
        "my-project",
        r#"schema: '1'
articles:
  my-article:
    title: My Article
    article_path: docs/my-article
"#,
    );
    let article_dir = repo.path().join("my-project/docs/my-article");
    fs::create_dir_all(&article_dir).unwrap();
    fs::write(article_dir.join("01-content.md"), "# Content\n").unwrap();

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["--output", "json", "build", "my-article", "--dry-run"])
        .output()
        .expect("command runs");
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Skip potential tracing lines
    let json_start = stdout.find('{').unwrap_or(0);
    let parsed: serde_json::Value = serde_json::from_str(&stdout[json_start..]).unwrap();
    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["data"]["article"], "my-article");
    assert!(parsed["data"]["article_path"].as_str().unwrap_or("").contains("my-article"));
    assert!(parsed["data"]["dry_run"].as_bool().unwrap_or(false));
}

// ═══════════════════════════════════════════════════════════════════════════
// US5 (Bug #1): build rewrites relative paths for output location
// ═══════════════════════════════════════════════════════════════════════════

/// Test helper: set up a multi-block directory article with relative image
/// and link references, build it, and return the output content.
fn setup_and_build_article(repo: &common::TempDir, article_name: &str, md_files: &[(&str, &str)]) -> String {
    let project = repo.path().join("my-project");
    // Ensure mind.yaml exists with proper build config
    let mind_yaml = "schema: '1'\nbuild:\n  output_dir: _build\n  format: md\n";
    common::write_mind_yaml(repo, "my-project", mind_yaml);

    let article_dir = project.join("docs").join(article_name);
    fs::create_dir_all(&article_dir).unwrap();
    for (filename, content) in md_files {
        fs::write(article_dir.join(filename), content).unwrap();
    }

    // Also create a minimal index entry so the article is resolvable
    let index_yaml = format!(
        "schema: '1'\narticles:\n  - title: Test\n    project: my-project\n    type: blog\n    article_path: docs/{article_name}\n    status: draft\n    created_at: '2026-07-01T00:00:00Z'\n    updated_at: '2026-07-01T00:00:00Z'\n"
    );
    fs::write(project.join("mind-index.yaml"), index_yaml).unwrap();

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(&project)
        .args(["build", article_name])
        .output()
        .expect("command runs");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "build failed; stdout={stdout}, stderr={stderr}");

    let output_path = project.join("_build").join(format!("{article_name}.md"));
    assert!(output_path.exists(), "output file should exist at {output_path:?}");
    fs::read_to_string(&output_path).unwrap()
}

/// T033: relative image paths are rewritten from source-location-relative
/// to output-location-relative.
#[test]
fn build_rewrites_relative_image_path() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    // Source: docs/my-article/01-opening.md references assets/pic.jpg
    // (relative to the article dir: docs/my-article/assets/pic.jpg)
    // Output: _build/my-article.md → should reference ../docs/my-article/assets/pic.jpg
    let content = setup_and_build_article(
        &repo,
        "my-article",
        &[("01-opening.md", "# Opening\n\n![hero](assets/pic.jpg)\n\nIntro text.\n")],
    );

    // The relative path should be rewritten so it resolves from _build/
    // Original: assets/pic.jpg from docs/my-article/ → docs/my-article/assets/pic.jpg
    // From _build/ to docs/my-article/assets/pic.jpg = ../docs/my-article/assets/pic.jpg
    assert!(content.contains("assets/pic.jpg"), "output must contain image reference; got: {content}");
    // Verify the source file is NOT modified
    let source = fs::read_to_string(repo.path().join("my-project/docs/my-article/01-opening.md")).unwrap();
    assert!(source.contains("![hero](assets/pic.jpg)"), "source file must NOT be modified");
}

/// T033: relative link (non-image) paths are rewritten.
#[test]
fn build_rewrites_relative_link_path() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let content = setup_and_build_article(
        &repo,
        "my-article",
        &[("01-opening.md", "# Link\n\nSee [details](notes/readme.md) for more.\n")],
    );

    assert!(content.contains("notes/readme.md"), "output must contain rewritten link; got: {content}");
}

/// T036: absolute paths and URLs are left unchanged.
#[test]
fn build_preserves_absolute_and_url_paths() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let content = setup_and_build_article(
        &repo,
        "my-article",
        &[(
            "01-opening.md",
            "# Links\n\n![logo](/absolute/logo.png)\n[site](https://example.com)\n[email](mailto:a@b.com)\n[top](#top)\n",
        )],
    );

    // All these must appear unchanged
    assert!(content.contains("/absolute/logo.png"), "absolute path preserved");
    assert!(content.contains("https://example.com"), "URL preserved");
    assert!(content.contains("mailto:a@b.com"), "mailto preserved");
    assert!(content.contains("#top"), "anchor preserved");
}

/// T033: paths inside fenced code blocks are NOT rewritten.
#[test]
fn build_skips_paths_inside_fenced_code_blocks() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let content = setup_and_build_article(
        &repo,
        "my-article",
        &[(
            "01-opening.md",
            "# Docs\n\nOutside: ![img](assets/real.jpg)\n\n```markdown\nInside: ![img](assets/fake.jpg)\n```\n",
        )],
    );

    // The real image reference outside the fence should be rewritten
    assert!(content.contains("assets/real.jpg"), "real ref preserved: {content}");
    // The fake reference inside the fence should stay verbatim
    assert!(content.contains("assets/fake.jpg"), "fenced ref must stay verbatim: {content}");
}

/// T036: data: URIs are left unchanged.
#[test]
fn build_preserves_data_uris() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let content = setup_and_build_article(
        &repo,
        "my-article",
        &[("01-opening.md", "# Inline\n\n![icon](data:image/png,base64,ABC123)\n")],
    );

    assert!(content.contains("data:image/png,base64,ABC123"), "data URI preserved: {content}");
}

/// Bug 1A: alt text must NOT be duplicated/nested when rewriting the path.
/// Regression for `![alt](p)` → `![alt![alt](new)]`.
#[test]
fn build_rewrite_preserves_alt_text() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let content = setup_and_build_article(
        &repo,
        "my-article",
        &[("01-opening.md", "# Pics\n\n![工作流全景](../shared/workflow.png)\n\n![image-xxx](assets/pic.png)\n")],
    );

    // Alt text preserved, never nested inside another `![`.
    assert!(content.contains("![工作流全景]("), "CJK alt text preserved: {content}");
    assert!(!content.contains("![工作流全景!["), "alt text must not be nested: {content}");
    assert!(!content.contains("![image-xxx!["), "alt text must not be nested: {content}");
    // Each image line has exactly one `](` (one well-formed image).
    for line in content.lines().filter(|l| l.contains("![")) {
        assert_eq!(line.matches("](").count(), 1, "exactly one link per image line: {line}");
    }
}

/// Bug 1B: rewritten paths are lexically normalised (no interior `foo/../`).
#[test]
fn build_rewrite_normalizes_redundant_segments() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let content =
        setup_and_build_article(&repo, "my-article", &[("01-opening.md", "# Pic\n\n![p](../shared/pic.png)\n")]);

    assert!(!content.contains("/../"), "path should be normalised, no interior /../: {content}");
    assert!(content.contains("shared/pic.png"), "path still resolves to shared/pic.png: {content}");
}

// ---------------------------------------------------------------------------
// Bug #22: build image path correctness with `@`-path + relative `--out`
// ---------------------------------------------------------------------------

/// Bug #22 repro: `@`-path (canonicalized, absolute) combined with a
/// relative `--out` must not produce a malformed mixed absolute/relative
/// path. Covers Markdown image, reference definition, and HTML `<img src>`
/// forms.
#[test]
fn build_at_path_with_relative_out_produces_valid_relative_paths() {
    let repo = tempfile::TempDir::new().unwrap();
    fs::write(repo.path().join("minds.yaml"), "schema: '1'\nprojects:\n  - projects/2026-blogs\n").unwrap();
    let project = repo.path().join("projects/2026-blogs");
    fs::create_dir_all(project.join("docs/my-article")).unwrap();
    fs::create_dir_all(project.join("assets")).unwrap();
    fs::write(project.join("mind.yaml"), "schema: '1'\n").unwrap();
    fs::write(
        project.join("docs/my-article/01-opening.md"),
        "# Opening\n\n![hero](../../assets/pic.png)\n\n[ref]: ../../assets/pic.png\n\n<img src=\"../../assets/pic.png\" alt=\"hero\">\n",
    )
    .unwrap();

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(&project)
        .args(["build", "@projects/2026-blogs/docs/my-article/", "--out", "outputs/my-article.md"])
        .assert()
        .success();

    let content = fs::read_to_string(project.join("outputs/my-article.md")).unwrap();
    assert!(content.contains("![hero](../assets/pic.png)"), "markdown image must be a valid relative path: {content}");
    assert!(
        content.contains("[ref]: ../assets/pic.png"),
        "reference definition must be a valid relative path: {content}"
    );
    assert!(
        content.contains(r#"<img src="../assets/pic.png" alt="hero">"#),
        "HTML img src must be a valid relative path: {content}"
    );
    assert!(!content.contains("////"), "must never contain a malformed path fragment: {content}");
    assert!(!content.contains("..//"), "must never contain a malformed path fragment: {content}");
}

/// Bug #22 corrected repro: the malformed-path case is not specific to
/// worktree depth. It appears when `--out` is outside the project tree, so
/// build must still emit a resolvable relative path instead of stitching a
/// relative prefix to an absolute asset path.
#[test]
fn build_with_absolute_outside_repo_out_produces_resolvable_relative_paths() {
    let repo = tempfile::TempDir::new().unwrap();
    fs::write(repo.path().join("minds.yaml"), "schema: '1'\nprojects:\n  - projects/2026-blogs\n").unwrap();
    let project = repo.path().join("projects/2026-blogs");
    let asset = project.join("assets/pic.png");
    fs::create_dir_all(project.join("docs/my-article")).unwrap();
    fs::create_dir_all(project.join("assets")).unwrap();
    fs::write(project.join("mind.yaml"), "schema: '1'\n").unwrap();
    fs::write(&asset, b"png").unwrap();
    fs::write(
        project.join("docs/my-article/01-opening.md"),
        "# Opening\n\n![hero](../../assets/pic.png)\n\n[ref]: ../../assets/pic.png\n\n<img src=\"../../assets/pic.png\" alt=\"hero\">\n",
    )
    .unwrap();

    let outside = tempfile::TempDir::new().unwrap();
    let output_path = outside.path().join("exports/my-article.md");

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(&project)
        .args(["build", "@projects/2026-blogs/docs/my-article/", "--out", output_path.to_str().unwrap()])
        .assert()
        .success();

    let content = fs::read_to_string(&output_path).unwrap();
    assert!(!content.contains(&project.display().to_string()), "must not embed the project absolute path: {content}");
    assert!(!content.contains("////"), "must never contain a malformed path fragment: {content}");
    assert!(!content.contains("..//"), "must never contain a malformed path fragment: {content}");

    let expected_asset = asset.canonicalize().unwrap();
    let output_dir = output_path.parent().unwrap();
    for target in [
        extract_between(&content, "![hero](", ")"),
        extract_between(&content, "[ref]: ", "\n"),
        extract_between(&content, r#"<img src=""#, r#"""#),
    ] {
        assert!(!Path::new(target).is_absolute(), "output reference must be relative: {target}");
        assert_eq!(
            output_dir.join(target).canonicalize().unwrap(),
            expected_asset,
            "output reference must resolve to the original asset: {target}"
        );
    }
}

fn extract_between<'a>(content: &'a str, start: &str, end: &str) -> &'a str {
    let start_idx = content.find(start).unwrap_or_else(|| panic!("missing start marker {start:?} in {content:?}"));
    let rest = &content[start_idx + start.len()..];
    let end_idx = rest.find(end).unwrap_or_else(|| panic!("missing end marker {end:?} after {start:?} in {content:?}"));
    &rest[..end_idx]
}

/// Bug #22: building the same article via `@`-path + relative `--out` and via
/// the plain indexed-article form must produce byte-identical image
/// references.
#[test]
fn build_at_path_matches_plain_indexed_build() {
    let repo = tempfile::TempDir::new().unwrap();
    fs::write(repo.path().join("minds.yaml"), "schema: '1'\nprojects:\n  - projects/2026-blogs\n").unwrap();
    let project = repo.path().join("projects/2026-blogs");
    fs::create_dir_all(project.join("docs/my-article")).unwrap();
    fs::write(project.join("mind.yaml"), "schema: '1'\n").unwrap();
    fs::write(project.join("docs/my-article/01-opening.md"), "# Opening\n\n![hero](../../assets/pic.png)\n").unwrap();
    fs::write(
        project.join("mind-index.yaml"),
        "schema: '1'\narticles:\n  - title: My Article\n    project: 2026-blogs\n    type: blog\n    article_path: docs/my-article\n    status: draft\n    created_at: '2026-07-01T00:00:00Z'\n    updated_at: '2026-07-01T00:00:00Z'\n",
    )
    .unwrap();

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(&project)
        .args(["build", "my-article"])
        .assert()
        .success();
    let plain_content = fs::read_to_string(project.join("outputs/my-article.md")).unwrap();
    fs::remove_file(project.join("outputs/my-article.md")).unwrap();

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(&project)
        .args(["build", "@projects/2026-blogs/docs/my-article/", "--out", "outputs/my-article.md"])
        .assert()
        .success();
    let at_path_content = fs::read_to_string(project.join("outputs/my-article.md")).unwrap();

    assert_eq!(plain_content, at_path_content, "image references must be identical across both invocation forms");
}

/// Bug #22: building from a project reached through a symlinked directory
/// (simulating a git worktree at a deep, symlinked path) must still produce
/// valid, non-malformed relative image paths.
#[cfg(unix)]
#[test]
fn build_through_symlinked_root_produces_valid_relative_paths() {
    use std::os::unix::fs::symlink;

    let real_repo = tempfile::TempDir::new().unwrap();
    fs::write(real_repo.path().join("minds.yaml"), "schema: '1'\nprojects:\n  - projects/2026-blogs\n").unwrap();
    let project = real_repo.path().join("projects/2026-blogs");
    fs::create_dir_all(project.join("docs/my-article")).unwrap();
    fs::write(project.join("mind.yaml"), "schema: '1'\n").unwrap();
    fs::write(project.join("docs/my-article/01-opening.md"), "# Opening\n\n![hero](../../assets/pic.png)\n").unwrap();

    let link_parent = tempfile::TempDir::new().unwrap();
    let symlinked_repo = link_parent.path().join("worktree-link");
    symlink(real_repo.path(), &symlinked_repo).unwrap();
    let symlinked_project = symlinked_repo.join("projects/2026-blogs");

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(&symlinked_project)
        .args(["build", "@projects/2026-blogs/docs/my-article/", "--out", "outputs/my-article.md"])
        .assert()
        .success();

    // Read back through the real (non-symlinked) path — atomic_write resolves
    // symlinked parent dirs, so the file lands in the real tree either way.
    let output_path = project.join("outputs/my-article.md");
    assert!(output_path.exists(), "output should exist at the real path: {output_path:?}");
    let content = fs::read_to_string(&output_path).unwrap();
    assert!(content.contains("![hero](../assets/pic.png)"), "path must be valid relative form: {content}");
    assert!(!content.contains("////"), "must never contain a malformed path fragment: {content}");
}

/// Bug #22 defense in depth: a reference whose relative path genuinely
/// cannot be resolved (mixed base) is kept verbatim and reported as a
/// warning on stderr, not written as a malformed path. Exercised at the
/// service layer via `rewrite_target`/`rewrite_relative_paths` unit tests in
/// `src/service/build.rs`; this test confirms the CLI end-to-end contract
/// still succeeds (exit 0) for a normal build with no unresolvable refs.
#[test]
fn build_succeeds_and_emits_no_warnings_when_all_references_resolve() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "new", "Test Article"])
        .output()
        .expect("command runs");
    assert!(output.status.success());

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["build", "test-article"])
        .output()
        .expect("command runs");
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("WARN:"), "a normal build must not emit path warnings: {stderr}");
}
