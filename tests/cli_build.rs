use assert_cmd::Command;
use predicates::str;
use std::fs;

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
    // Indexed entry source_path does not exist — the article key is "review" and
    // its source_path is a directory that does exist
    common::write_index(
        &repo,
        "my-project",
        r#"schema: '1'
articles:
  review:
    title: Review
    source_path: docs/review
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
// US2: Build with source directory tests
// ---------------------------------------------------------------------------

#[test]
fn build_from_configured_source_dir_under_docs() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_mind_yaml(
        &repo,
        "my-project",
        "schema: '1'\nbuild:\n  articles:\n    my-article:\n      source_dir: docs/custom-src\n",
    );
    // Create the custom source directory
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
    assert!(content.contains("Intro"), "should include content from configured source dir");
    assert!(content.contains("Body"), "should include all files from configured source dir");
}

#[test]
fn build_from_configured_source_dir_outside_docs() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_mind_yaml(
        &repo,
        "my-project",
        "schema: '1'\nbuild:\n  articles:\n    my-article:\n      source_dir: specs\n",
    );
    // Create source directory outside docs/
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
fn build_with_missing_source_dir_fails() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_mind_yaml(
        &repo,
        "my-project",
        "schema: '1'\nbuild:\n  articles:\n    my-article:\n      source_dir: non-existent-dir\n",
    );
    common::write_article_index(&repo, "my-project", "my-article");

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["build", "my-article"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(2), "missing source_dir should be usage error");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("non-existent-dir") || stderr.contains("does not exist"),
        "error should name the missing dir"
    );
    assert!(stderr.contains("my-article"), "error should name the article");
}

#[test]
fn build_default_source_dir_unchanged_without_config() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    // No source_dir config — uses default behavior
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
// US2: Build auto-indexing (via configured source_dir)
// ---------------------------------------------------------------------------

#[test]
fn build_auto_indexes_article_in_mind_index_yaml() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    // Configure a source_dir for the article so build can find it without the index
    common::write_mind_yaml(
        &repo,
        "my-project",
        "schema: '1'\nbuild:\n  articles:\n    auto-index-me:\n      source_dir: specs\n",
    );
    // Create article file in the configured source_dir
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
        .args(["--format", "json", "article", "list"])
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
    let auto_indexed = articles.iter().find(|a| a["source_path"].as_str().unwrap_or("").contains("auto-index-me"));
    assert!(auto_indexed.is_some(), "built article should appear in index after build: {stdout}");
}

#[test]
fn build_auto_indexes_configured_source_dir_as_article_directory() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_mind_yaml(
        &repo,
        "my-project",
        "schema: '1'\nbuild:\n  articles:\n    quarterly-review:\n      source_dir: specs/quarterly\n",
    );
    let source_dir = repo.path().join("my-project/specs/quarterly");
    std::fs::create_dir_all(&source_dir).unwrap();
    std::fs::write(source_dir.join("01-intro.md"), "Intro\n").unwrap();
    std::fs::write(source_dir.join("02-body.md"), "Body\n").unwrap();
    common::write_index(&repo, "my-project", "schema: '1'\n");

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["build", "quarterly-review"])
        .assert()
        .success();

    let index = std::fs::read_to_string(repo.path().join("my-project/mind-index.yaml")).unwrap();
    assert!(index.contains("source_path: specs/quarterly"), "index should store the configured source dir: {index}");
    assert!(
        !index.contains("source_path: specs/quarterly/quarterly-review.md"),
        "index should not invent a Markdown file inside the configured source dir: {index}"
    );
}

// ---------------------------------------------------------------------------
// US2: Build resolves by article key using indexed source path (T019–T022)
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
fn build_dry_run_input_source_includes_directory_file() {
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
fn build_title_not_used_for_source_path_derivation() {
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
    source_path: docs/team-updates
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
fn build_dry_run_json_envelope_includes_source_path() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_index(
        &repo,
        "my-project",
        r#"schema: '1'
articles:
  my-article:
    title: My Article
    source_path: docs/my-article
"#,
    );
    let article_dir = repo.path().join("my-project/docs/my-article");
    fs::create_dir_all(&article_dir).unwrap();
    fs::write(article_dir.join("01-content.md"), "# Content\n").unwrap();

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["--format", "json", "build", "my-article", "--dry-run"])
        .output()
        .expect("command runs");
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Skip potential tracing lines
    let json_start = stdout.find('{').unwrap_or(0);
    let parsed: serde_json::Value = serde_json::from_str(&stdout[json_start..]).unwrap();
    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["data"]["article"], "my-article");
    assert!(parsed["data"]["source_path"].as_str().unwrap_or("").contains("my-article"));
    assert!(parsed["data"]["dry_run"].as_bool().unwrap_or(false));
}
