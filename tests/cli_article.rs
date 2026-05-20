use assert_cmd::Command;

mod common;

#[test]
fn article_new_creates_file_and_index() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "new", "My First Article"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(0), "article new should succeed");

    let md_path = repo.path().join("my-project/docs/my-first-article/01-opening.md");
    assert!(md_path.exists(), "article file should exist");

    let index_path = repo.path().join("my-project/mind-index.yaml");
    assert!(index_path.exists(), "mind-index.yaml should exist");

    let index_content = std::fs::read_to_string(index_path).unwrap();
    assert!(index_content.contains("My First Article"));
    assert!(index_content.contains("my-first-article"));
}

#[test]
fn article_new_refuses_duplicate() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "new", "Test"])
        .assert()
        .success();

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "new", "Test"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(1), "duplicate should fail");
}

#[test]
fn article_new_needs_project() {
    let repo = common::setup_repo();

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path())
        .args(["article", "new", "Hello"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(2), "should fail without project context");
}

#[test]
fn article_new_with_project_flag() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path())
        .args(["article", "new", "Hello", "--project", "my-project"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(0), "should succeed with --project flag");
}

#[test]
fn article_new_preserves_mind_index_dictionary_shape() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_index(
        &repo,
        "my-project",
        r#"schema: '1'
project: my-project
updated: 2026-05-01
articles:
  existing:
    title: Existing
    source_path: docs/existing
"#,
    );

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "new", "New Article"])
        .assert()
        .success();

    let content = std::fs::read_to_string(repo.path().join("my-project/mind-index.yaml")).unwrap();
    assert!(content.contains("schema:"), "schema alias should be preserved:\n{content}");
    assert!(!content.contains("schema_version:"), "schema_version should not be introduced:\n{content}");
    assert!(content.contains("project: my-project"), "unknown top-level fields should remain:\n{content}");
    assert!(content.contains("docs/existing:"), "existing article key should be path全名:\n{content}");
    assert!(content.contains("docs/new-article:"), "new article key should be path全名:\n{content}");
    assert!(!content.contains("extra:"), "unknown fields should not be nested under extra:\n{content}");
}

#[test]
fn article_list_shows_articles() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "new", "Article One"])
        .assert()
        .success();

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "new", "Article Two"])
        .assert()
        .success();

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "list"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Article One"));
    assert!(stdout.contains("Article Two"));
}

#[test]
fn article_list_empty() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "list"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("No articles found"));
}

// ---------------------------------------------------------------------------
// article lint tests
// ---------------------------------------------------------------------------

#[test]
fn article_lint_no_issues() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "new", "valid-article"])
        .assert()
        .success();

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "lint"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("No issues found"));
}

#[test]
fn article_lint_detects_bad_filename() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    // Create a file with non-conforming name
    let docs = repo.path().join("my-project/docs");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(docs.join("Bad File Name.md"), "# content").unwrap();

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "lint"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("filename_convention"));
}

#[test]
fn article_lint_fix_renames_file() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let docs = repo.path().join("my-project/docs");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(docs.join("Bad File.md"), "# content").unwrap();

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "lint", "--fix"])
        .output()
        .expect("command runs");
    if output.status.code() != Some(0) {
        let stderr = String::from_utf8(output.stderr).unwrap_or_default();
        let stdout = String::from_utf8(output.stdout).unwrap_or_default();
        panic!("lint --fix failed (exit {:?})\nstderr: {stderr}\nstdout: {stdout}", output.status.code());
    }

    // Original file should be renamed
    assert!(!docs.join("Bad File.md").exists());
    assert!(docs.join("bad-file.md").exists());
}

#[test]
fn article_lint_empty_file() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let docs = repo.path().join("my-project/docs");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(docs.join("empty.md"), "").unwrap();

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "lint"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("empty_file"));
}

#[test]
fn article_lint_json_output() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let docs = repo.path().join("my-project/docs");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(docs.join("Bad File.md"), "# content").unwrap();

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["--format", "json", "article", "lint"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["status"], "ok");
    assert!(parsed["data"].is_array());
}

// ---------------------------------------------------------------------------
// article index tests
// ---------------------------------------------------------------------------

fn json_index(args: &[&str], cwd: &std::path::Path) -> (serde_json::Value, std::process::Output) {
    let mut all = vec!["--format", "json", "article", "index"];
    all.extend_from_slice(args);
    let output =
        Command::cargo_bin("mf").expect("binary exists").current_dir(cwd).args(&all).output().expect("command runs");
    let out = output.stdout.clone();
    let stdout = String::from_utf8(out).unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("JSON parse error: {e}\nstdout: {stdout:?}"));
    (parsed, output)
}

#[test]
fn article_index_dry_run_shows_changes() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    // Create an article as a single file so the index diff sees it.
    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "new", "Indexed Article", "--file"])
        .assert()
        .success();

    // Dry-run should report no changes since index is in sync
    let (parsed, output) = json_index(&["--dry-run"], &repo.path().join("my-project"));
    assert_eq!(output.status.code(), Some(0), "dry-run should succeed");
    assert_eq!(parsed["data"]["dry_run"], true);
    assert_eq!(parsed["data"]["added"].as_array().map(|a| a.len()), Some(0));
    assert_eq!(parsed["data"]["removed"].as_array().map(|r| r.len()), Some(0));
}

#[test]
fn article_index_adds_new_file() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    // Manually create a markdown file in docs/
    let docs = repo.path().join("my-project/docs");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(docs.join("manual-article.md"), "# Manual Article\ncontent").unwrap();

    // Run index — should pick up the new file
    let (parsed, output) = json_index(&[], &repo.path().join("my-project"));
    assert_eq!(output.status.code(), Some(0));
    assert!(parsed["data"]["articles_count"].as_u64().unwrap_or(0) >= 1);

    // Verify the article appears in list
    let list_output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "list"])
        .output()
        .expect("command runs");
    let list_stdout = String::from_utf8(list_output.stdout).unwrap();
    assert!(list_stdout.contains("manual article")); // title derived from filename
}

#[test]
fn article_index_removes_deleted_file() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    // Create article via command
    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "new", "To Be Removed"])
        .assert()
        .success();

    // Delete the directory article
    let dir_path = repo.path().join("my-project/docs/to-be-removed");
    std::fs::remove_dir_all(&dir_path).unwrap();

    // Run index — should remove from index
    let (_, output) = json_index(&[], &repo.path().join("my-project"));
    assert_eq!(output.status.code(), Some(0));

    // Article should no longer appear in list
    let list_output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "list"])
        .output()
        .expect("command runs");
    let list_stdout = String::from_utf8(list_output.stdout).unwrap();
    assert!(!list_stdout.contains("To Be Removed"));
}

#[test]
fn article_index_empty_docs() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let (parsed, output) = json_index(&[], &repo.path().join("my-project"));
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(parsed["data"]["articles_count"], 0);
}

#[test]
fn article_index_needs_project() {
    let repo = common::setup_repo();
    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path())
        .args(["article", "index"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(2), "should fail without project context");
}

#[test]
fn article_index_with_project_flag() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    // Create a file manually
    let docs = repo.path().join("my-project/docs");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(docs.join("flagged-article.md"), "# Flagged\ncontent").unwrap();

    let (parsed, output) = json_index(&["--project", "my-project"], repo.path());
    assert_eq!(output.status.code(), Some(0), "should succeed with --project flag");
    assert!(parsed["data"]["articles_count"].as_u64().unwrap_or(0) >= 1);
}

// ---------------------------------------------------------------------------
// US2: Article source directory tests
// ---------------------------------------------------------------------------

#[test]
fn article_list_shows_default_source_dir() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "new", "Default Dir"])
        .assert()
        .success();

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "list"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    // Should show the default source_dir: docs/default-dir
    assert!(stdout.contains("docs/default-dir"), "default source_dir should be docs/<article-name>: {stdout}");
}

#[test]
fn article_list_json_shows_source_dir_field() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "new", "JSON Article"])
        .assert()
        .success();

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["--format", "json", "article", "list"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["status"], "ok");
    let articles = parsed["data"]["articles"].as_array().unwrap();
    assert!(!articles.is_empty());
    let first = &articles[0];
    // The JSON should include a source_dir field per contract
    assert!(first.get("source_dir").is_some(), "article JSON should include source_dir field");
    let source_dir = first["source_dir"].as_str().unwrap_or("");
    assert!(source_dir.contains("docs/"), "default source_dir should contain docs/: {source_dir}");
}

#[test]
fn article_list_json_with_configured_source_dir() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_article_index(&repo, "my-project", "custom-article");
    common::write_doc(&repo, "my-project", "custom-article", "content\n");
    // Configure a custom source_dir for the article
    common::write_mind_yaml(
        &repo,
        "my-project",
        "schema: '1'\nbuild:\n  articles:\n    custom-article:\n      source_dir: specs\n",
    );

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["--format", "json", "article", "list"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let articles = parsed["data"]["articles"].as_array().unwrap();
    let article = articles.iter().find(|a| a["source_path"].as_str().unwrap_or("").contains("custom-article"));
    assert!(article.is_some(), "custom-article should appear in listing");
    let source_dir = article.unwrap()["source_dir"].as_str().unwrap_or("");
    assert_eq!(source_dir, "specs", "source_dir should reflect configured value: {source_dir}");
}

// ---------------------------------------------------------------------------
// US2: Article discovery consistency tests
// ---------------------------------------------------------------------------

#[test]
fn article_index_scans_configured_source_dir() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_mind_yaml(
        &repo,
        "my-project",
        "schema: '1'\nbuild:\n  articles:\n    my-article:\n      source_dir: specs\n",
    );
    common::write_source_file(&repo, "my-project", "specs", "my-article", "# Custom article\n");
    common::write_doc(&repo, "my-project", "docs-article", "# Docs article\n");

    let (parsed, output) = json_index(&[], &repo.path().join("my-project"));
    assert_eq!(output.status.code(), Some(0), "index should succeed");
    assert!(
        parsed["data"]["articles_count"].as_u64().unwrap_or(0) >= 2,
        "should index articles from both docs/ and specs/"
    );

    let list_output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "list"])
        .output()
        .expect("command runs");
    let stdout = String::from_utf8(list_output.stdout).unwrap();
    assert!(stdout.contains("my-article"), "should list article from source_dir: {stdout}");
    assert!(stdout.contains("docs-article"), "should list article from docs: {stdout}");
}

#[test]
fn article_list_shows_articles_after_indexing_no_preexisting_index() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    common::write_doc(&repo, "my-project", "indexed-article", "# Indexed\n");

    let (_, output) = json_index(&[], &repo.path().join("my-project"));
    assert_eq!(output.status.code(), Some(0), "index should succeed");

    let list_output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "list"])
        .output()
        .expect("command runs");
    let stdout = String::from_utf8(list_output.stdout).unwrap();
    assert!(stdout.contains("indexed-article"), "should list indexed article: {stdout}");
}

#[test]
fn article_list_shows_mind_yaml_articles_without_manual_indexing() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_mind_yaml(
        &repo,
        "my-project",
        "schema: '1'\nbuild:\n  articles:\n    configured-article:\n      source_dir: specs\n",
    );
    common::write_source_file(&repo, "my-project", "specs", "configured-article", "# Configured\n");

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "list"])
        .output()
        .expect("command runs");

    assert_eq!(output.status.code(), Some(0), "article list should succeed without manual index");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("configured-article"), "should list configured article from mind.yaml: {stdout}");
    assert!(stdout.contains("specs/configured-article.md"), "should show configured source file: {stdout}");
}

#[test]
fn article_index_uses_configured_article_key_for_directory_source_dir() {
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

    let (_, output) = json_index(&[], &repo.path().join("my-project"));
    assert_eq!(output.status.code(), Some(0), "article index should succeed");

    let list_output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["--format", "json", "article", "list"])
        .output()
        .expect("command runs");
    let stdout = String::from_utf8(list_output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let articles = parsed["data"]["articles"].as_array().unwrap();
    let article = articles.iter().find(|a| a["source_path"] == "specs/quarterly").expect("configured article");

    assert_eq!(article["title"], "quarterly review");
    assert_eq!(article["source_dir"], "specs/quarterly");
    assert!(
        !articles.iter().any(|a| a["source_path"] == "specs/quarterly/01-intro.md"),
        "directory source_dir should be indexed as the configured article, not each part: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// US3: Informative warning detail tests
// ---------------------------------------------------------------------------

#[test]
fn article_list_duplicate_key_warning_includes_detail() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_index(
        &repo,
        "my-project",
        r#"schema: '1'
articles:
  first:
    title: First
    source_path: docs/first.md
articles:
  second:
    title: Second
    source_path: docs/second.md
"#,
    );

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "list"])
        .output()
        .expect("command runs");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Warning should include duplicate key detail
    let full_output = format!("{stderr}{stdout}");
    assert!(
        full_output.contains("duplicate top-level key"),
        "warning should mention 'duplicate top-level key': {full_output}"
    );
}

#[test]
fn article_index_duplicate_key_warning_includes_key_name() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_index(
        &repo,
        "my-project",
        r#"schema: '1'
articles:
  first:
    title: First
    source_path: docs/first.md
articles:
  second:
    title: Second
    source_path: docs/second.md
"#,
    );

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "list"])
        .output()
        .expect("command runs");

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let full_output = format!("{stderr}{stdout}");
    // Warning should mention the specific key name
    assert!(
        full_output.contains("articles") || full_output.contains("'articles'"),
        "warning should include key name 'articles': {full_output}"
    );
}

#[test]
fn article_index_same_article_in_docs_and_source_dir_dedup() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    // Configure a source_dir that matches the default docs name
    // This tests the dedup when the same article appears in both places
    common::write_mind_yaml(
        &repo,
        "my-project",
        "schema: '1'\nbuild:\n  articles:\n    dedup-test:\n      source_dir: docs\n",
    );
    // Create the file in docs/ (which is also the configured source_dir)
    common::write_doc(&repo, "my-project", "dedup-test", "# Dedup\n");

    let (parsed, output) = json_index(&[], &repo.path().join("my-project"));
    assert_eq!(output.status.code(), Some(0));
    let articles_count = parsed["data"]["articles_count"].as_u64().unwrap_or(0);
    assert_eq!(articles_count, 1, "should index article exactly once despite duplicate source_dir config: {parsed}");
}

#[test]
fn article_index_skips_missing_source_dir() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_mind_yaml(
        &repo,
        "my-project",
        "schema: '1'\nbuild:\n  articles:\n    ghost:\n      source_dir: non-existent\n",
    );
    common::write_doc(&repo, "my-project", "real-article", "# Real\n");

    let (parsed, output) = json_index(&[], &repo.path().join("my-project"));
    assert_eq!(output.status.code(), Some(0), "index should succeed even with missing source_dir");
    assert!(parsed["data"]["articles_count"].as_u64().unwrap_or(0) >= 1);
}

// ---------------------------------------------------------------------------
// US2: Generated article discovery tests (T015)
// ---------------------------------------------------------------------------

#[test]
fn list_discovers_generated_articles() {
    let repo = common::scaffold_project_with_template(
        "my-project",
        "daily_report",
        "outputs/{date:YYYY-MM}/{date:YYYY-MM-DD}.md",
        "generated",
        &["outputs/2026-05/2026-05-15.md"],
    );

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["--format", "json", "article", "list"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(0), "list should succeed");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["status"], "ok");

    let articles = parsed["data"]["articles"].as_array().unwrap();
    let gen = articles.iter().find(|a| a["title"] == "daily_report/2026-05-15");
    assert!(gen.is_some(), "should find generated article: {stdout}");

    let gen = gen.unwrap();
    assert_eq!(gen["source_path"], "outputs/2026-05/2026-05-15.md");
    assert_eq!(gen["template_origin"]["template_name"], "daily_report");
    assert_eq!(gen["template_origin"]["slot_value"], "2026-05-15");
}

#[test]
fn list_persists_index_after_index_command() {
    let repo = common::scaffold_project_with_template(
        "my-project",
        "daily_report",
        "outputs/{date:YYYY-MM}/{date:YYYY-MM-DD}.md",
        "generated",
        &["outputs/2026-05/2026-05-15.md"],
    );

    // Run article index to persist the generated article
    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "index"])
        .assert()
        .success();

    // Verify mind-index.yaml contains the generated article
    let index_path = repo.path().join("my-project/mind-index.yaml");
    let content = std::fs::read_to_string(&index_path).unwrap();
    assert!(content.contains("daily_report/2026-05-15"), "index should contain template article id: {content}");
    assert!(content.contains("outputs/2026-05/2026-05-15.md"), "index should contain template source path: {content}");
}

#[test]
fn list_is_byte_idempotent() {
    let repo = common::scaffold_project_with_template(
        "my-project",
        "daily_report",
        "outputs/{date:YYYY-MM}/{date:YYYY-MM-DD}.md",
        "generated",
        &["outputs/2026-05/2026-05-15.md"],
    );

    let run = |name: &str| -> String {
        let output = Command::cargo_bin("mf")
            .expect("binary exists")
            .current_dir(repo.path().join("my-project"))
            .args(["--format", "json", "article", "list"])
            .output()
            .expect("command runs");
        assert_eq!(output.status.code(), Some(0), "{name} should succeed");
        String::from_utf8(output.stdout).unwrap()
    };

    let first = run("first call");
    let second = run("second call");
    assert_eq!(first, second, "consecutive list calls should produce identical JSON");
}

#[test]
fn list_works_without_prior_index() {
    let repo = common::scaffold_project_with_template(
        "my-project",
        "daily_report",
        "outputs/{date:YYYY-MM}/{date:YYYY-MM-DD}.md",
        "generated",
        &["outputs/2026-05/2026-05-15.md"],
    );

    // Delete mind-index.yaml
    let index_path = repo.path().join("my-project/mind-index.yaml");
    if index_path.exists() {
        std::fs::remove_file(&index_path).unwrap();
    }

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["--format", "json", "article", "list"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(0), "list should succeed without prior index");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["status"], "ok");

    let articles = parsed["data"]["articles"].as_array().unwrap();
    assert!(
        articles.iter().any(|a| a["title"] == "daily_report/2026-05-15"),
        "should still discover generated article without index: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// US1: Fix Article Source Path Identity — integration tests (T010–T013)
// ---------------------------------------------------------------------------

#[test]
fn article_index_declared_directory_source_path() {
    let repo = common::scaffold_team_reports_minimal_repro();
    let project_path = repo.path().join("team-reports");

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(&project_path)
        .args(["article", "index"])
        .output()
        .expect("command runs");
    assert_eq!(
        output.status.code(),
        Some(0),
        "index should succeed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let articles = common::read_index_articles_map(&repo, "team-reports");
    common::assert_article_source_path(&articles, "docs/2026-05-monthly", "docs/2026-05-monthly");
}

#[test]
fn article_index_does_not_write_nonexistent_md_file() {
    let repo = common::scaffold_team_reports_minimal_repro();
    let project_path = repo.path().join("team-reports");

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(&project_path)
        .args(["article", "index"])
        .assert()
        .success();

    let articles = common::read_index_articles_map(&repo, "team-reports");
    // The article's source_path must be the directory, not a fake .md file
    common::assert_article_source_path(&articles, "docs/2026-05-monthly", "docs/2026-05-monthly");
    // Verify the fake .md file does not exist on disk
    assert!(
        !project_path.join("docs/2026-05-monthly.md").exists(),
        "fake docs/2026-05-monthly.md should not exist on disk or in index"
    );
}

#[test]
fn article_index_deterministic_source_path_repeatable() {
    let repo = common::scaffold_team_reports_minimal_repro();
    let project_path = repo.path().join("team-reports");

    let run_index = || -> String {
        let output = Command::cargo_bin("mf")
            .expect("binary exists")
            .current_dir(&project_path)
            .args(["article", "index", "-p", "team-reports"])
            .output()
            .expect("command runs");
        assert_eq!(output.status.code(), Some(0));
        std::fs::read_to_string(project_path.join("mind-index.yaml")).unwrap()
    };

    let first = run_index();
    let second = run_index();
    assert_eq!(first, second, "re-index should produce byte-identical mind-index.yaml");
    assert!(
        first.contains("source_path: docs/2026-05-monthly"),
        "first run should have directory source_path: {first}"
    );
}

#[test]
fn article_index_missing_declared_source_diagnostic() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    // Declare an article with no existing source path
    common::write_mind_yaml(&repo, "my-project", "schema: '1'\nbuild:\n  articles:\n    ghost-article: {}\n");
    // No docs/ghost-article/ dir, no docs/ghost-article.md file

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "index"])
        .output()
        .expect("command runs");
    // Index should still succeed — FR-003: missing-source articles are still emitted
    // using the conventional docs/<key>.md fallback (visible to `mf article list`).
    assert_eq!(output.status.code(), Some(0), "index should succeed");

    let articles = common::read_index_articles_map(&repo, "my-project");
    // The article should be in the index with the conventional docs/ path
    // (not a random invented path). Key is path全名: docs/ghost-article
    let entry = articles.get("docs/ghost-article").expect("docs/ghost-article should be in index");
    let sp = entry["source_path"].as_str().unwrap_or("");
    assert_eq!(sp, "docs/ghost-article.md", "missing source should use conventional path");

    // FR-003: a stderr warning must name the missing source so the user can fix it.
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ghost-article"), "stderr must name the missing article: {stderr}");
    assert!(stderr.contains("docs/ghost-article.md"), "stderr must name the expected source path: {stderr}");
}
