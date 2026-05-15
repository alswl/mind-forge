use assert_cmd::Command;

mod common;

#[test]
fn article_new_creates_file_and_index() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "new", "blog", "My First Article"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(0), "article new should succeed");

    let md_path = repo.path().join("my-project/docs/my-first-article.md");
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
        .args(["article", "new", "blog", "Test"])
        .assert()
        .success();

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "new", "blog", "Test"])
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
        .args(["article", "new", "blog", "Hello"])
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
        .args(["article", "new", "blog", "Hello", "--project", "my-project"])
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
        .args(["article", "new", "blog", "New Article"])
        .assert()
        .success();

    let content = std::fs::read_to_string(repo.path().join("my-project/mind-index.yaml")).unwrap();
    assert!(content.contains("schema:"), "schema alias should be preserved:\n{content}");
    assert!(!content.contains("schema_version:"), "schema_version should not be introduced:\n{content}");
    assert!(content.contains("project: my-project"), "unknown top-level fields should remain:\n{content}");
    assert!(content.contains("articles:\n  existing:"), "articles should remain a mapping:\n{content}");
    assert!(content.contains("  new-article:"), "new article should be inserted as a mapping entry:\n{content}");
    assert!(!content.contains("extra:"), "unknown fields should not be nested under extra:\n{content}");
}

#[test]
fn article_list_shows_articles() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "new", "blog", "Article One"])
        .assert()
        .success();

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "new", "blog", "Article Two"])
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
        .args(["article", "new", "blog", "valid-article"])
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

    // Create an article via normal command
    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "new", "blog", "Indexed Article"])
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
        .args(["article", "new", "blog", "To Be Removed"])
        .assert()
        .success();

    // Delete the file
    let md_path = repo.path().join("my-project/docs/to-be-removed.md");
    std::fs::remove_file(&md_path).unwrap();

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
        .args(["article", "new", "blog", "Default Dir"])
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
        .args(["article", "new", "blog", "JSON Article"])
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
    let articles = parsed["data"].as_array().unwrap();
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
    let articles = parsed["data"].as_array().unwrap();
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
