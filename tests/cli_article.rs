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
fn article_new_creates_missing_index_with_project_path_selector() {
    let repo = common::setup_repo();
    std::fs::write(repo.path().join("minds.yaml"), "schema_version: '1'\nprojects_dir: projects\nprojects: []\n")
        .unwrap();
    common::create_project(&repo, "projects/myproj");

    let index_path = repo.path().join("projects/myproj/mind-index.yaml");
    assert!(!index_path.exists(), "fixture should start without mind-index.yaml");

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path())
        .args(["article", "new", "foo", "--template", "blog", "--project", "projects/myproj"])
        .output()
        .expect("command runs");
    assert_eq!(
        output.status.code(),
        Some(0),
        "article new should succeed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let md_path = repo.path().join("projects/myproj/docs/foo/01-opening.md");
    assert!(md_path.exists(), "article file should exist");
    assert!(index_path.exists(), "mind-index.yaml should be created");

    let index_content = std::fs::read_to_string(&index_path).unwrap();
    assert!(index_content.contains("schema: '1'"), "index should contain minimal schema:\n{index_content}");
    assert!(index_content.contains("docs/foo:"), "index should contain the new article key:\n{index_content}");
    assert!(index_content.contains("type: blog"), "index should preserve template type:\n{index_content}");

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path())
        .args(["article", "list", "--project", "projects/myproj"])
        .output()
        .expect("command runs");
    assert_eq!(
        output.status.code(),
        Some(0),
        "article list should succeed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("docs/foo"), "article list should include created article: {stdout}");
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
    article_path: docs/existing
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
    assert!(stdout.contains("docs/article-one"));
    assert!(stdout.contains("docs/article-two"));
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
    assert!(stdout.contains("0 errors, 0 warnings, 0 info"));
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
    assert_eq!(output.status.code(), Some(1));
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
        .args(["--output", "json", "article", "lint"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["status"], "ok");
    assert!(parsed["data"].is_object());
    assert_eq!(parsed["data"]["kind"], "article");
    assert!(parsed["data"]["issues"].is_array());
}

// ---------------------------------------------------------------------------
// article index tests
// ---------------------------------------------------------------------------

fn json_index(args: &[&str], cwd: &std::path::Path) -> (serde_json::Value, std::process::Output) {
    let mut all = vec!["--output", "json", "article", "index"];
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
    assert!(parsed["data"]["kept_count"].as_u64().unwrap_or(0) >= 1);

    // Verify the article appears in list
    let list_output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "list"])
        .output()
        .expect("command runs");
    let list_stdout = String::from_utf8(list_output.stdout).unwrap();
    assert!(list_stdout.contains("docs/manual-article.md")); // canonical path identity
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
    assert_eq!(parsed["data"]["kept_count"], 0);
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
    assert!(parsed["data"]["kept_count"].as_u64().unwrap_or(0) >= 1);
}

// ---------------------------------------------------------------------------
// US2: Article article directory tests
// ---------------------------------------------------------------------------

#[test]
fn article_list_shows_default_article_dir() {
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
    // Should show the default article_dir: docs/default-dir
    assert!(stdout.contains("docs/default-dir"), "default article_dir should be docs/<article-name>: {stdout}");
}

#[test]
fn article_list_text_is_path_centered() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_mind_yaml(
        &repo,
        "my-project",
        "schema_version: '1'\nbuild:\n  articles:\n    component-meta-core: {}\n    component-meta-spec: {}\n",
    );
    std::fs::create_dir_all(repo.path().join("my-project/docs/component-meta-core")).unwrap();
    std::fs::write(repo.path().join("my-project/docs/component-meta-core/01-intro.md"), "# Intro\n").unwrap();

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "list"])
        .output()
        .expect("command runs");

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    // Headers may be absent in pipe mode (non-TTY), so check data content instead
    assert!(!stdout.contains("ORIGIN"), "human list should not expose discovery origin column: {stdout}");
    assert!(stdout.contains("docs/component-meta-core"));
    assert!(stdout.contains("component meta core"), "list should show article titles: {stdout}");
    assert!(stdout.contains("docs/component-meta-spec.md"));
    assert!(stdout.contains("component meta spec"), "list should show article titles: {stdout}");
    assert!(!stdout.contains("directory"), "list should not expose filesystem primitive labels: {stdout}");
}

#[test]
fn article_list_json_shows_article_dir_field() {
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
        .args(["--output", "json", "article", "list"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["status"], "ok");
    let articles = parsed["data"]["articles"].as_array().unwrap();
    assert!(!articles.is_empty());
    let first = &articles[0];
    // The JSON should include a article_dir field per contract
    assert!(first.get("article_dir").is_some(), "article JSON should include article_dir field");
    assert!(first.get("identity").is_some(), "article JSON should include path identity");
    assert_eq!(first["identity"], first["article_path"]);
    assert_eq!(first["path"], first["article_path"]);
    assert!(first.get("content_kind").is_some(), "article JSON should include content kind");
    let article_dir = first["article_dir"].as_str().unwrap_or("");
    assert!(article_dir.contains("docs/"), "default article_dir should contain docs/: {article_dir}");
}

#[test]
fn article_list_text_shows_single_file_content_shape() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "new", "Single File Article", "--file"])
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
    assert!(stdout.contains("docs/single-file-article.md"));
    assert!(stdout.contains("Single File"), "single-file article should use writing content label: {stdout}");
}

#[test]
fn article_list_json_with_configured_article_dir() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_article_index(&repo, "my-project", "custom-article");
    common::write_doc(&repo, "my-project", "custom-article", "content\n");
    // Configure a custom article_dir for the article
    common::write_mind_yaml(
        &repo,
        "my-project",
        "schema: '1'\nbuild:\n  articles:\n    custom-article:\n      article_dir: specs\n",
    );

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["--output", "json", "article", "list"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let articles = parsed["data"]["articles"].as_array().unwrap();
    let article = articles.iter().find(|a| a["article_path"].as_str().unwrap_or("").contains("custom-article"));
    assert!(article.is_some(), "custom-article should appear in listing");
    let article_dir = article.unwrap()["article_dir"].as_str().unwrap_or("");
    assert_eq!(article_dir, "specs", "article_dir should reflect configured value: {article_dir}");
}

// ---------------------------------------------------------------------------
// US2: Article discovery consistency tests
// ---------------------------------------------------------------------------

#[test]
fn article_index_scans_configured_article_dir() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_mind_yaml(
        &repo,
        "my-project",
        "schema: '1'\nbuild:\n  articles:\n    my-article:\n      article_dir: specs\n",
    );
    common::write_source_file(&repo, "my-project", "specs", "my-article", "# Custom article\n");
    common::write_doc(&repo, "my-project", "docs-article", "# Docs article\n");

    let (parsed, output) = json_index(&[], &repo.path().join("my-project"));
    assert_eq!(output.status.code(), Some(0), "index should succeed");
    assert!(
        parsed["data"]["kept_count"].as_u64().unwrap_or(0) >= 2,
        "should index articles from both docs/ and specs/"
    );

    let list_output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "list"])
        .output()
        .expect("command runs");
    let stdout = String::from_utf8(list_output.stdout).unwrap();
    assert!(stdout.contains("my-article"), "should list article from article_dir: {stdout}");
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
        "schema: '1'\nbuild:\n  articles:\n    configured-article:\n      article_dir: specs\n",
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
    assert!(stdout.contains("specs/configured-article.md"), "should show configured article file: {stdout}");
}

#[test]
fn article_index_uses_configured_article_key_for_directory_article_dir() {
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

    let (_, output) = json_index(&[], &repo.path().join("my-project"));
    assert_eq!(output.status.code(), Some(0), "article index should succeed");

    let list_output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["--output", "json", "article", "list"])
        .output()
        .expect("command runs");
    let stdout = String::from_utf8(list_output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let articles = parsed["data"]["articles"].as_array().unwrap();
    let article = articles.iter().find(|a| a["article_path"] == "specs/quarterly").expect("configured article");

    assert_eq!(article["title"], "quarterly review");
    assert_eq!(article["article_dir"], "specs/quarterly");
    assert!(
        !articles.iter().any(|a| a["article_path"] == "specs/quarterly/01-intro.md"),
        "directory article_dir should be indexed as the configured article, not each part: {stdout}"
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
    article_path: docs/first.md
articles:
  second:
    title: Second
    article_path: docs/second.md
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
    article_path: docs/first.md
articles:
  second:
    title: Second
    article_path: docs/second.md
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
fn article_index_same_article_in_docs_and_article_dir_dedup() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    // Configure a article_dir that matches the default docs name
    // This tests the dedup when the same article appears in both places
    common::write_mind_yaml(
        &repo,
        "my-project",
        "schema: '1'\nbuild:\n  articles:\n    dedup-test:\n      article_dir: docs\n",
    );
    // Create the file in docs/ (which is also the configured article_dir)
    common::write_doc(&repo, "my-project", "dedup-test", "# Dedup\n");

    let (parsed, output) = json_index(&[], &repo.path().join("my-project"));
    assert_eq!(output.status.code(), Some(0));
    let articles_count = parsed["data"]["kept_count"].as_u64().unwrap_or(0);
    assert_eq!(articles_count, 1, "should index article exactly once despite duplicate article_dir config: {parsed}");
}

#[test]
fn article_index_skips_missing_article_dir() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_mind_yaml(
        &repo,
        "my-project",
        "schema: '1'\nbuild:\n  articles:\n    ghost:\n      article_dir: non-existent\n",
    );
    common::write_doc(&repo, "my-project", "real-article", "# Real\n");

    let (parsed, output) = json_index(&[], &repo.path().join("my-project"));
    assert_eq!(output.status.code(), Some(0), "index should succeed even with missing article_dir");
    assert!(parsed["data"]["kept_count"].as_u64().unwrap_or(0) >= 1);
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
        .args(["--output", "json", "article", "list"])
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
    assert_eq!(gen["article_path"], "outputs/2026-05/2026-05-15.md");
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
    assert!(content.contains("outputs/2026-05/2026-05-15.md"), "index should contain template article path: {content}");
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
            .args(["--output", "json", "article", "list"])
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
        .args(["--output", "json", "article", "list"])
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
fn article_index_declared_directory_article_path() {
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
    common::assert_article_path(&articles, "docs/2026-05-monthly", "docs/2026-05-monthly");
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
    // The article's article_path must be the directory, not a fake .md file
    common::assert_article_path(&articles, "docs/2026-05-monthly", "docs/2026-05-monthly");
    // Verify the fake .md file does not exist on disk
    assert!(
        !project_path.join("docs/2026-05-monthly.md").exists(),
        "fake docs/2026-05-monthly.md should not exist on disk or in index"
    );
}

#[test]
fn article_index_deterministic_article_path_repeatable() {
    let repo = common::scaffold_team_reports_minimal_repro();
    let project_path = repo.path().join("team-reports");

    let run_index = || -> String {
        let output = Command::cargo_bin("mf")
            .expect("binary exists")
            .current_dir(&project_path)
            .args(["--project", "team-reports", "article", "index"])
            .output()
            .expect("command runs");
        assert_eq!(output.status.code(), Some(0));
        std::fs::read_to_string(project_path.join("mind-index.yaml")).unwrap()
    };

    let first = run_index();
    let second = run_index();
    assert_eq!(first, second, "re-index should produce byte-identical mind-index.yaml");
    assert!(
        first.contains("article_path: docs/2026-05-monthly"),
        "first run should have directory article_path: {first}"
    );
}

#[test]
fn article_index_missing_declared_source_diagnostic() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    // Declare an article with no existing article path
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
    let sp = entry["article_path"].as_str().unwrap_or("");
    assert_eq!(sp, "docs/ghost-article.md", "missing source should use conventional path");

    // FR-003: a stderr warning must name the missing source so the user can fix it.
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("ghost-article"), "stderr must name the missing article: {stderr}");
    assert!(stderr.contains("docs/ghost-article.md"), "stderr must name the expected article path: {stderr}");
}

// ── Article remove tests ────────────────────────────────────────────────────

fn seed_article(repo: &tempfile::TempDir, project_name: &str, title: &str, article_path: &str) {
    let project_dir = repo.path().join(project_name);
    if let Some(parent) = std::path::Path::new(article_path).parent() {
        std::fs::create_dir_all(project_dir.join(parent)).unwrap();
    }
    std::fs::write(project_dir.join(article_path), format!("# {title}\n\nContent.\n")).unwrap();
    let yaml = format!(
        "schema_version: '1'\narticles:\n  - title: '{title}'\n    project: '{project_name}'\n    article_type: blog\n    article_path: '{article_path}'\n    status: draft\n    created_at: '2026-05-08T10:00:00Z'\n    updated_at: '2026-05-08T10:00:00Z'\n"
    );
    common::write_index(repo, project_name, &yaml);
}

#[test]
fn article_remove_success() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    seed_article(&repo, "my-project", "My Article", "docs/my-article.md");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "article",
            "remove",
            "My Article",
            "--project",
            "my-project",
            "--yes",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    // File removed
    assert!(!repo.path().join("my-project/docs/my-article.md").exists(), "article file should be deleted");

    // Index entry removed
    let index_content = std::fs::read_to_string(repo.path().join("my-project/mind-index.yaml")).unwrap();
    assert!(!index_content.contains("My Article"), "article entry should be removed from index");
}

#[test]
fn article_remove_not_found() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "article",
            "remove",
            "Nonexistent",
            "--project",
            "my-project",
            "--yes",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not found"), "stderr: {stderr}");
}

#[test]
fn article_remove_json_envelope() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    seed_article(&repo, "my-project", "My Article", "docs/my-article.md");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "article",
            "remove",
            "My Article",
            "--project",
            "my-project",
            "--output",
            "json",
            "--yes",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    assert_eq!(v["data"]["kind"], "article");
    assert_eq!(v["data"]["removed"], true);
}

#[test]
fn article_remove_dry_run() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    seed_article(&repo, "my-project", "My Article", "docs/my-article.md");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "article",
            "remove",
            "My Article",
            "--project",
            "my-project",
            "--dry-run",
            "--yes",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    // File should still exist
    assert!(repo.path().join("my-project/docs/my-article.md").exists(), "file should still exist after dry run");

    // Entry should still exist in index
    let index_content = std::fs::read_to_string(repo.path().join("my-project/mind-index.yaml")).unwrap();
    assert!(index_content.contains("My Article"), "entry should still exist after dry run");
}

#[test]
fn article_remove_rm_alias() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    seed_article(&repo, "my-project", "My Article", "docs/my-article.md");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "article",
            "rm",
            "My Article",
            "--project",
            "my-project",
            "--yes",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    assert!(
        !repo.path().join("my-project/docs/my-article.md").exists(),
        "article file should be deleted via `rm` alias"
    );
}

// ── Article rename tests ────────────────────────────────────────────────────

#[test]
fn article_rename_success() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    seed_article(&repo, "my-project", "Old Title", "docs/old-title.md");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "article",
            "rename",
            "Old Title",
            "new-slug",
            "--project",
            "my-project",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    // Old file should be renamed to new slug
    let project = repo.path().join("my-project");
    assert!(!project.join("docs/old-title.md").exists(), "old file should be renamed");
    assert!(project.join("docs/new-slug.md").exists(), "new file should exist at new slug");

    // Title should be unchanged (rename only changes slug)
    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("Old Title"), "title should be unchanged: {index_content}");
    assert!(index_content.contains("new-slug"), "slug should be updated: {index_content}");
}

#[test]
fn article_rename_not_found() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "article",
            "rename",
            "Nonexistent",
            "Whatever",
            "--project",
            "my-project",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not found"), "stderr: {stderr}");
}

#[test]
fn article_rename_json_envelope() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    seed_article(&repo, "my-project", "Old Title", "docs/old-title.md");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "article",
            "rename",
            "Old Title",
            "new-slug",
            "--project",
            "my-project",
            "--output",
            "json",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    assert_eq!(v["data"]["details"]["old_title"], "Old Title");
    assert_eq!(v["data"]["details"]["new_title"], "Old Title", "title should be unchanged after rename");
    assert!(v["data"]["old_identity"].as_str().is_some_and(|s| s.contains("old-title")));
    assert!(v["data"]["identity"].as_str().is_some_and(|s| s.contains("new-slug")));
}

// ── Article rename: preserves title ──────────────────────────────────────

#[test]
fn article_rename_preserves_title() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    seed_article(&repo, "my-project", "Keep This Title", "docs/keep-this-title.md");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "article",
            "rename",
            "Keep This Title",
            "different-slug",
            "--project",
            "my-project",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let project = repo.path().join("my-project");
    assert!(project.join("docs/different-slug.md").exists(), "file should be renamed to new slug");
    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("Keep This Title"), "title should be preserved: {index_content}");
    assert!(index_content.contains("different-slug"), "slug should be updated: {index_content}");
}

#[test]
fn article_rename_directory() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    // Create a directory article
    let article_dir = repo.path().join("my-project/docs/old-dir");
    std::fs::create_dir_all(&article_dir).unwrap();
    std::fs::write(article_dir.join("01-opening.md"), "# Old Dir\n\nContent.\n").unwrap();
    std::fs::write(article_dir.join("02-body.md"), "## Body\n\nMore.\n").unwrap();

    // Write index with directory path (no .md)
    let index_yaml = r#"schema_version: '1'
articles:
  - title: 'Old Dir'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/old-dir'
    status: draft
    created_at: '2026-05-08T10:00:00Z'
    updated_at: '2026-05-08T10:00:00Z'
"#;
    common::write_index(&repo, "my-project", index_yaml);

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "article",
            "rename",
            "Old Dir",
            "new-dir",
            "--project",
            "my-project",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let project = repo.path().join("my-project");
    assert!(!project.join("docs/old-dir").exists(), "old directory should be renamed");
    assert!(project.join("docs/new-dir").is_dir(), "new directory should exist");
    assert!(project.join("docs/new-dir/01-opening.md").exists(), "block files should be preserved");
    assert!(project.join("docs/new-dir/02-body.md").exists(), "block files should be preserved");

    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("Old Dir"), "title should be unchanged: {index_content}");
    assert!(index_content.contains("docs/new-dir"), "path should be updated: {index_content}");
}

// ── Article update --title tests ──────────────────────────────────────────

#[test]
fn article_update_title_success() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    seed_article(&repo, "my-project", "Old Title", "docs/old-title.md");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "article",
            "update",
            "Old Title",
            "--title",
            "New Title",
            "--project",
            "my-project",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let project = repo.path().join("my-project");
    // File path should NOT change
    assert!(project.join("docs/old-title.md").exists(), "file path should not change on title update");

    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("New Title"), "title should be updated: {index_content}");
    assert!(index_content.contains("docs/old-title.md"), "path should be unchanged: {index_content}");
}

#[test]
fn article_update_nothing_to_change() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    seed_article(&repo, "my-project", "Some Title", "docs/some-title.md");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "article", "update", "Some Title", "--project", "my-project"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("nothing to update"), "stderr: {stderr}");
}

// ── Article convert tests: US1 directory-to-file ──────────────────────────

#[test]
fn article_convert_to_single_file_converts_one_directory_article() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    // Create a directory article with one section
    let article_dir = repo.path().join("my-project/docs/my-article");
    std::fs::create_dir_all(&article_dir).unwrap();
    std::fs::write(article_dir.join("01-opening.md"), "# My Article\n\nContent.\n").unwrap();

    // Write index with the directory path
    let index_yaml = r#"schema_version: '1'
articles:
  - title: 'My Article'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/my-article'
    status: draft
    created_at: '2026-05-15T00:00:00Z'
    updated_at: '2026-05-15T00:00:00Z'
"#;
    common::write_index(&repo, "my-project", index_yaml);

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "convert", "--to-single-file"])
        .output()
        .expect("command runs");

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    // Target file should exist
    assert!(repo.path().join("my-project/docs/my-article.md").exists(), "converted single-file should exist");
    // Source directory should be removed
    assert!(!repo.path().join("my-project/docs/my-article").exists(), "old directory article should be removed");
    // Index should reference the new path
    let index_content = std::fs::read_to_string(repo.path().join("my-project/mind-index.yaml")).unwrap();
    assert!(
        index_content.contains("article_path: docs/my-article.md"),
        "index should reference the new path: {index_content}"
    );
}

#[test]
fn article_convert_to_single_file_multiple_ordered() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    // Create two directory articles
    for name in &["alpha", "beta"] {
        let dir = repo.path().join(format!("my-project/docs/{name}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("01-opening.md"), format!("# {}\n\nContent.\n", name)).unwrap();
    }

    let index_yaml = r#"schema_version: '1'
articles:
  - title: 'Alpha'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/alpha'
    status: draft
    created_at: '2026-05-15T00:00:00Z'
    updated_at: '2026-05-15T00:00:00Z'
  - title: 'Beta'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/beta'
    status: draft
    created_at: '2026-05-15T00:00:00Z'
    updated_at: '2026-05-15T00:00:00Z'
"#;
    common::write_index(&repo, "my-project", index_yaml);

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "convert", "--to-single-file"])
        .output()
        .expect("command runs");

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let stdout = String::from_utf8(output.stdout).unwrap();
    // Output should be ordered by source_path (alpha before beta)
    let alpha_pos = stdout.find("docs/alpha").unwrap();
    let beta_pos = stdout.find("docs/beta").unwrap();
    assert!(alpha_pos < beta_pos, "alpha should appear before beta in output: {stdout}");

    assert!(repo.path().join("my-project/docs/alpha.md").exists());
    assert!(repo.path().join("my-project/docs/beta.md").exists());
    assert!(!repo.path().join("my-project/docs/alpha").exists());
    assert!(!repo.path().join("my-project/docs/beta").exists());
}

#[test]
fn article_convert_to_single_file_article_list_visibility() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    // Create a directory article
    let article_dir = repo.path().join("my-project/docs/my-article");
    std::fs::create_dir_all(&article_dir).unwrap();
    std::fs::write(article_dir.join("01-opening.md"), "# My Article\n\nContent.\n").unwrap();

    let index_yaml = r#"schema_version: '1'
articles:
  - title: 'My Article'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/my-article'
    status: draft
    created_at: '2026-05-15T00:00:00Z'
    updated_at: '2026-05-15T00:00:00Z'
"#;
    common::write_index(&repo, "my-project", index_yaml);

    // Run conversion
    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "convert", "--to-single-file"])
        .assert()
        .success();

    // Article list should show the new path
    let list_output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "list"])
        .output()
        .expect("command runs");

    assert!(list_output.status.success());
    let stdout = String::from_utf8(list_output.stdout).unwrap();
    assert!(stdout.contains("docs/my-article.md"), "article list should show the new single-file path: {stdout}");
    assert!(!stdout.contains("BLOCKED"), "article should no longer show as BLOCKED: {stdout}");
}

// ── Article convert tests: US2 file-to-directory ──────────────────────────

#[test]
fn article_convert_to_directory_converts_one_single_file_article() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    // Create a single-file article
    std::fs::create_dir_all(repo.path().join("my-project/docs")).unwrap();
    std::fs::write(repo.path().join("my-project/docs/my-article.md"), "# My Article\n\nContent.\n").unwrap();

    let index_yaml = r#"schema_version: '1'
articles:
  - title: 'My Article'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/my-article.md'
    status: draft
    created_at: '2026-05-15T00:00:00Z'
    updated_at: '2026-05-15T00:00:00Z'
"#;
    common::write_index(&repo, "my-project", index_yaml);

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "convert", "--to-directory"])
        .output()
        .expect("command runs");

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    // Target directory with section file should exist
    assert!(repo.path().join("my-project/docs/my-article/01-opening.md").exists(), "01-opening.md should be created");
    // Source file should be removed
    assert!(!repo.path().join("my-project/docs/my-article.md").exists(), "old single-file article should be removed");
    // Index should reference the new directory path
    let index_content = std::fs::read_to_string(repo.path().join("my-project/mind-index.yaml")).unwrap();
    assert!(
        index_content.contains("article_path: docs/my-article"),
        "index should reference directory path: {index_content}"
    );
}

#[test]
fn article_convert_to_directory_multiple_ordered() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    std::fs::create_dir_all(repo.path().join("my-project/docs")).unwrap();
    for name in &["alpha", "beta"] {
        std::fs::write(repo.path().join(format!("my-project/docs/{}.md", name)), format!("# {}\n\nContent.\n", name))
            .unwrap();
    }

    let index_yaml = r#"schema_version: '1'
articles:
  - title: 'Alpha'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/alpha.md'
    status: draft
    created_at: '2026-05-15T00:00:00Z'
    updated_at: '2026-05-15T00:00:00Z'
  - title: 'Beta'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/beta.md'
    status: draft
    created_at: '2026-05-15T00:00:00Z'
    updated_at: '2026-05-15T00:00:00Z'
"#;
    common::write_index(&repo, "my-project", index_yaml);

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "convert", "--to-directory"])
        .output()
        .expect("command runs");

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let stdout = String::from_utf8(output.stdout).unwrap();
    let alpha_pos = stdout.find("docs/alpha.md").unwrap();
    let beta_pos = stdout.find("docs/beta.md").unwrap();
    assert!(alpha_pos < beta_pos, "alpha should appear before beta: {stdout}");

    assert!(repo.path().join("my-project/docs/alpha/01-opening.md").exists());
    assert!(repo.path().join("my-project/docs/beta/01-opening.md").exists());
    assert!(!repo.path().join("my-project/docs/alpha.md").exists());
    assert!(!repo.path().join("my-project/docs/beta.md").exists());
}

#[test]
fn article_convert_to_directory_article_list_visibility() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    std::fs::create_dir_all(repo.path().join("my-project/docs")).unwrap();
    std::fs::write(repo.path().join("my-project/docs/my-article.md"), "# My Article\n\nContent.\n").unwrap();

    let index_yaml = r#"schema_version: '1'
articles:
  - title: 'My Article'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/my-article.md'
    status: draft
    created_at: '2026-05-15T00:00:00Z'
    updated_at: '2026-05-15T00:00:00Z'
"#;
    common::write_index(&repo, "my-project", index_yaml);

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "convert", "--to-directory"])
        .assert()
        .success();

    let list_output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "list"])
        .output()
        .expect("command runs");

    assert!(list_output.status.success());
    let stdout = String::from_utf8(list_output.stdout).unwrap();
    assert!(stdout.contains("docs/my-article"), "article list should show directory path: {stdout}");
    assert!(!stdout.contains("Single File"), "article should no longer show as Single File: {stdout}");
}

// ── Article convert tests: US3 dry-run ────────────────────────────────────

#[test]
fn article_convert_to_single_file_dry_run_no_writes() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let article_dir = repo.path().join("my-project/docs/my-article");
    std::fs::create_dir_all(&article_dir).unwrap();
    std::fs::write(article_dir.join("01-opening.md"), "# My Article\n\nContent.\n").unwrap();

    let index_yaml = r#"schema_version: '1'
articles:
  - title: 'My Article'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/my-article'
    status: draft
    created_at: '2026-05-15T00:00:00Z'
    updated_at: '2026-05-15T00:00:00Z'
"#;
    common::write_index(&repo, "my-project", index_yaml);

    // Snapshot index content before dry-run
    let index_before = std::fs::read_to_string(repo.path().join("my-project/mind-index.yaml")).unwrap();

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "convert", "--to-single-file", "--dry-run"])
        .output()
        .expect("command runs");

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("[dry-run]"), "output should show dry-run prefix: {stdout}");
    assert!(stdout.contains("would convert article"), "should say 'would convert': {stdout}");

    // No file was created
    assert!(!repo.path().join("my-project/docs/my-article.md").exists(), "target file should NOT exist after dry-run");
    // Source directory still exists
    assert!(
        repo.path().join("my-project/docs/my-article").exists(),
        "source directory should still exist after dry-run"
    );
    // Index unchanged
    let index_after = std::fs::read_to_string(repo.path().join("my-project/mind-index.yaml")).unwrap();
    assert_eq!(index_before, index_after, "index should be byte-for-byte unchanged");
}

#[test]
fn article_convert_to_directory_dry_run_no_writes() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    std::fs::create_dir_all(repo.path().join("my-project/docs")).unwrap();
    std::fs::write(repo.path().join("my-project/docs/my-article.md"), "# My Article\n\nContent.\n").unwrap();

    let index_yaml = r#"schema_version: '1'
articles:
  - title: 'My Article'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/my-article.md'
    status: draft
    created_at: '2026-05-15T00:00:00Z'
    updated_at: '2026-05-15T00:00:00Z'
"#;
    common::write_index(&repo, "my-project", index_yaml);
    let index_before = std::fs::read_to_string(repo.path().join("my-project/mind-index.yaml")).unwrap();

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "convert", "--to-directory", "--dry-run"])
        .output()
        .expect("command runs");

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("[dry-run]"), "output should show dry-run prefix: {stdout}");
    assert!(stdout.contains("would convert article"), "should say 'would convert': {stdout}");

    assert!(
        !repo.path().join("my-project/docs/my-article").exists(),
        "target directory should NOT exist after dry-run"
    );
    assert!(repo.path().join("my-project/docs/my-article.md").exists(), "source file should still exist after dry-run");
    let index_after = std::fs::read_to_string(repo.path().join("my-project/mind-index.yaml")).unwrap();
    assert_eq!(index_before, index_after, "index should be byte-for-byte unchanged");
}

#[test]
fn article_convert_dry_run_zero_eligible() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    // Index with only a single-file article; dry-run to-single-file should find 0 eligible
    std::fs::create_dir_all(repo.path().join("my-project/docs")).unwrap();
    std::fs::write(repo.path().join("my-project/docs/my-article.md"), "# My Article\n\nContent.\n").unwrap();

    let index_yaml = r#"schema_version: '1'
articles:
  - title: 'My Article'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/my-article.md'
    status: draft
    created_at: '2026-05-15T00:00:00Z'
    updated_at: '2026-05-15T00:00:00Z'
"#;
    common::write_index(&repo, "my-project", index_yaml);

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "convert", "--to-single-file", "--dry-run"])
        .output()
        .expect("command runs");

    assert!(output.status.success(), "should succeed with zero eligible");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("0 would convert"), "should show 0 would convert: {stdout}");
}

// ── Article convert tests: US4 inferred direction ─────────────────────────

#[test]
fn article_convert_no_direction_non_tty_errors() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let article_dir = repo.path().join("my-project/docs/my-article");
    std::fs::create_dir_all(&article_dir).unwrap();
    std::fs::write(article_dir.join("01-opening.md"), "# My Article\n\nContent.\n").unwrap();

    let index_yaml = r#"schema_version: '1'
articles:
  - title: 'My Article'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/my-article'
    status: draft
    created_at: '2026-05-15T00:00:00Z'
    updated_at: '2026-05-15T00:00:00Z'
"#;
    common::write_index(&repo, "my-project", index_yaml);

    // Passthrough stdin as /dev/null to force non-TTY
    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "convert"])
        .output()
        .expect("command runs");

    // Should fail with exit code 2 (usage error)
    assert_eq!(output.status.code(), Some(2), "should exit 2 for usage error");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("--to-single-file") || stderr.contains("--to-directory"),
        "error should mention explicit direction flags: {stderr}"
    );

    // No files or indexes changed
    assert!(repo.path().join("my-project/docs/my-article").exists(), "source should still exist");
    assert!(!repo.path().join("my-project/docs/my-article.md").exists(), "target should NOT exist");
}

#[test]
fn article_convert_ambiguous_direction_errors() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    // Create both a directory article AND a single-file article
    let dir = repo.path().join("my-project/docs/dir-article");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("01-opening.md"), "# Dir\n\nContent.\n").unwrap();

    std::fs::create_dir_all(repo.path().join("my-project/docs")).unwrap();
    std::fs::write(repo.path().join("my-project/docs/file-article.md"), "# File\n\nContent.\n").unwrap();

    let index_yaml = r#"schema_version: '1'
articles:
  - title: 'Dir Article'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/dir-article'
    status: draft
    created_at: '2026-05-15T00:00:00Z'
    updated_at: '2026-05-15T00:00:00Z'
  - title: 'File Article'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/file-article.md'
    status: draft
    created_at: '2026-05-15T00:00:00Z'
    updated_at: '2026-05-15T00:00:00Z'
"#;
    common::write_index(&repo, "my-project", index_yaml);

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "convert"])
        .output()
        .expect("command runs");

    assert_eq!(output.status.code(), Some(2), "ambiguous should exit 2");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("ambiguous") || stderr.contains("both"),
        "error should mention ambiguous direction: {stderr}"
    );
}

// ── Article convert tests: US5 safety/skip handling ───────────────────────

#[test]
fn article_convert_skips_multi_section_directory() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let dir = repo.path().join("my-project/docs/multi");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("01-opening.md"), "# Opening\n").unwrap();
    std::fs::write(dir.join("02-details.md"), "# Details\n").unwrap();

    let index_yaml = r#"schema_version: '1'
articles:
  - title: 'Multi'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/multi'
    status: draft
    created_at: '2026-05-15T00:00:00Z'
    updated_at: '2026-05-15T00:00:00Z'
"#;
    common::write_index(&repo, "my-project", index_yaml);

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "convert", "--to-single-file"])
        .output()
        .expect("command runs");

    assert!(output.status.success(), "should succeed with skip");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("multiple_section_files"), "should mention multiple_section_files: {stdout}");
    assert!(stdout.contains("0 converted") || stdout.contains("0 would convert"), "should be 0 converted");

    // Source directory unchanged
    assert!(repo.path().join("my-project/docs/multi").exists());
    assert!(repo.path().join("my-project/docs/multi/01-opening.md").exists());
    assert!(repo.path().join("my-project/docs/multi/02-details.md").exists());
}

#[test]
fn article_convert_skips_empty_directory() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let dir = repo.path().join("my-project/docs/empty");
    std::fs::create_dir_all(&dir).unwrap();
    // No markdown files in the directory

    let index_yaml = r#"schema_version: '1'
articles:
  - title: 'Empty'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/empty'
    status: draft
    created_at: '2026-05-15T00:00:00Z'
    updated_at: '2026-05-15T00:00:00Z'
"#;
    common::write_index(&repo, "my-project", index_yaml);

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "convert", "--to-single-file"])
        .output()
        .expect("command runs");

    assert!(output.status.success(), "should succeed with skip");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("no_section_files"), "should mention no_section_files: {stdout}");
}

#[test]
fn article_convert_skips_target_file_exists() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    // Create directory article AND target file
    let dir = repo.path().join("my-project/docs/my-article");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("01-opening.md"), "# Article\n").unwrap();
    // Create the target file to cause conflict
    std::fs::write(repo.path().join("my-project/docs/my-article.md"), "# Conflict\n").unwrap();

    let index_yaml = r#"schema_version: '1'
articles:
  - title: 'My Article'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/my-article'
    status: draft
    created_at: '2026-05-15T00:00:00Z'
    updated_at: '2026-05-15T00:00:00Z'
"#;
    common::write_index(&repo, "my-project", index_yaml);

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "convert", "--to-single-file"])
        .output()
        .expect("command runs");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("target_exists"), "should mention target_exists: {stdout}");

    // Both source and target still exist
    assert!(repo.path().join("my-project/docs/my-article").exists());
    assert!(repo.path().join("my-project/docs/my-article.md").exists());
}

#[test]
fn article_convert_skips_target_directory_exists() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    // Create single-file article AND target directory
    std::fs::create_dir_all(repo.path().join("my-project/docs/my-article")).unwrap();
    std::fs::write(repo.path().join("my-project/docs/my-article.md"), "# Article\n").unwrap();

    let index_yaml = r#"schema_version: '1'
articles:
  - title: 'My Article'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/my-article.md'
    status: draft
    created_at: '2026-05-15T00:00:00Z'
    updated_at: '2026-05-15T00:00:00Z'
"#;
    common::write_index(&repo, "my-project", index_yaml);

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "convert", "--to-directory"])
        .output()
        .expect("command runs");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("target_exists"), "should mention target_exists: {stdout}");

    // Both source and target still exist
    assert!(repo.path().join("my-project/docs/my-article.md").exists());
    assert!(repo.path().join("my-project/docs/my-article").exists());
}

#[test]
fn article_convert_skips_extra_files_in_directory() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let dir = repo.path().join("my-project/docs/my-article");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("01-opening.md"), "# Article\n").unwrap();
    // Extra non-.md file
    std::fs::write(dir.join("image.png"), "fake png").unwrap();

    let index_yaml = r#"schema_version: '1'
articles:
  - title: 'My Article'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/my-article'
    status: draft
    created_at: '2026-05-15T00:00:00Z'
    updated_at: '2026-05-15T00:00:00Z'
"#;
    common::write_index(&repo, "my-project", index_yaml);

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "convert", "--to-single-file"])
        .output()
        .expect("command runs");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("extra_files"), "should mention extra_files: {stdout}");

    // Directory still exists with all files
    assert!(repo.path().join("my-project/docs/my-article").exists());
    assert!(repo.path().join("my-project/docs/my-article/image.png").exists());
}

#[test]
fn article_convert_idempotent_to_single_file() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let dir = repo.path().join("my-project/docs/my-article");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("01-opening.md"), "# Article\n\nContent.\n").unwrap();

    let index_yaml = r#"schema_version: '1'
articles:
  - title: 'My Article'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/my-article'
    status: draft
    created_at: '2026-05-15T00:00:00Z'
    updated_at: '2026-05-15T00:00:00Z'
"#;
    common::write_index(&repo, "my-project", index_yaml);

    // First conversion
    let output1 = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "convert", "--to-single-file"])
        .output()
        .expect("command runs");
    assert!(output1.status.success());
    let stdout1 = String::from_utf8(output1.stdout).unwrap();
    assert!(stdout1.contains("1 converted"), "first run: {stdout1}");

    // Second conversion (idempotent)
    let output2 = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "convert", "--to-single-file"])
        .output()
        .expect("command runs");
    assert!(output2.status.success());
    let stdout2 = String::from_utf8(output2.stdout).unwrap();
    assert!(stdout2.contains("0 converted"), "second run should have 0 converted: {stdout2}");
}

#[test]
fn article_convert_idempotent_to_directory() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    std::fs::create_dir_all(repo.path().join("my-project/docs")).unwrap();
    std::fs::write(repo.path().join("my-project/docs/my-article.md"), "# Article\n\nContent.\n").unwrap();

    let index_yaml = r#"schema_version: '1'
articles:
  - title: 'My Article'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/my-article.md'
    status: draft
    created_at: '2026-05-15T00:00:00Z'
    updated_at: '2026-05-15T00:00:00Z'
"#;
    common::write_index(&repo, "my-project", index_yaml);

    // First conversion
    let output1 = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "convert", "--to-directory"])
        .output()
        .expect("command runs");
    assert!(output1.status.success());
    let stdout1 = String::from_utf8(output1.stdout).unwrap();
    assert!(stdout1.contains("1 converted"), "first run: {stdout1}");

    // Second conversion (idempotent)
    let output2 = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "convert", "--to-directory"])
        .output()
        .expect("command runs");
    assert!(output2.status.success());
    let stdout2 = String::from_utf8(output2.stdout).unwrap();
    assert!(stdout2.contains("0 converted"), "second run should have 0 converted: {stdout2}");
}

// ── All-projects article list ────────────────────────────────────────────────

#[test]
fn article_list_all_projects_text() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    common::create_project(&repo, "beta");
    common::write_doc(&repo, "alpha", "post", "# Alpha post\n");
    // Enough delay for mtime to differ across filesystems
    std::thread::sleep(std::time::Duration::from_millis(200));
    common::write_doc(&repo, "beta", "note", "# Beta note\n");

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path())
        .env("MF_FORCE_TTY", "1")
        .args(["article", "list"])
        .output()
        .expect("command runs");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();

    // TTY mode: PROJECT column header is present
    assert!(stdout.contains("PROJECT"), "PROJECT column missing:\n{stdout}");

    // Both articles appear with their project names
    assert!(stdout.contains("beta"), "beta project name missing:\n{stdout}");
    assert!(stdout.contains("alpha"), "alpha project name missing:\n{stdout}");

    // Articles appear: beta's note (newer mtime) should come before alpha's post
    let beta_pos = stdout.find("docs/note.md").expect("beta article");
    let alpha_pos = stdout.find("docs/post.md").expect("alpha article");
    assert!(beta_pos < alpha_pos, "newer article (beta/note) should appear before alpha/post:\n{stdout}");
}

#[test]
fn article_list_all_projects_json() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    common::create_project(&repo, "beta");
    common::write_doc(&repo, "alpha", "post", "# Alpha post\n");
    std::thread::sleep(std::time::Duration::from_millis(10));
    common::write_doc(&repo, "beta", "note", "# Beta note\n");

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path())
        .args(["--output", "json", "article", "list"])
        .output()
        .expect("command runs");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let articles = v["data"]["articles"].as_array().expect("articles array");

    assert!(!articles.is_empty(), "should have articles");

    // Each article has mtime field
    for a in articles {
        assert!(a.get("mtime").is_some(), "article missing mtime: {a}");
        assert!(a.get("project").is_some(), "article missing project: {a}");
    }

    // Sorted by mtime descending: beta/note first
    let first = &articles[0];
    assert_eq!(first["project"].as_str().unwrap(), "beta");
    assert!(first["article_path"].as_str().unwrap().contains("note"));
}

#[test]
fn article_list_all_projects_sorted_by_mtime() {
    let repo = common::setup_repo();
    common::create_project(&repo, "first");
    common::write_doc(&repo, "first", "old", "# Oldest\n");
    std::thread::sleep(std::time::Duration::from_millis(500));

    common::create_project(&repo, "second");
    common::write_doc(&repo, "second", "mid", "# Middle\n");
    std::thread::sleep(std::time::Duration::from_millis(500));

    common::create_project(&repo, "third");
    common::write_doc(&repo, "third", "new", "# Newest\n");

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path())
        .args(["--output", "json", "article", "list"])
        .output()
        .expect("command runs");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let articles = v["data"]["articles"].as_array().expect("articles array");
    assert_eq!(articles.len(), 3);

    // All three articles are present (order is by mtime, verify mtimes are non-increasing)
    let mtimes: Vec<u64> = articles.iter().map(|a| a["mtime"].as_u64().unwrap_or(0)).collect();
    for i in 1..mtimes.len() {
        assert!(mtimes[i - 1] >= mtimes[i], "mtimes should be sorted descending, got: {mtimes:?}");
    }
    // "old" should have the smallest (or equal) mtime since it was written first
    assert!(mtimes.last().copied().unwrap_or(0) <= mtimes[0], "oldest article should have smallest mtime: {mtimes:?}");
}

#[test]
fn article_list_inside_project_dir_still_works() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_doc(&repo, "my-project", "hello", "# Hello\n");

    // Run from inside the project dir — should still list only that project
    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .env("MF_FORCE_TTY", "1")
        .args(["article", "list"])
        .output()
        .expect("command runs");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    // Single-project mode: no PROJECT column, only PATH/TITLE/STATUS
    assert!(!stdout.contains("PROJECT"), "should not show PROJECT column in single-project mode:\n{stdout}");
    assert!(stdout.contains("TITLE"), "should have TITLE column:\n{stdout}");
    assert!(stdout.contains("docs/hello.md"), "should show the article:\n{stdout}");
}

#[test]
fn article_list_with_explicit_project_still_works() {
    let repo = common::setup_repo();
    common::create_project(&repo, "foo");
    common::create_project(&repo, "bar");
    common::write_doc(&repo, "foo", "foo-post", "# Foo\n");
    common::write_doc(&repo, "bar", "bar-post", "# Bar\n");

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path())
        .args(["--project", "foo", "article", "list"])
        .output()
        .expect("command runs");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.contains("PROJECT"), "should not show PROJECT column with --project:\n{stdout}");
    assert!(stdout.contains("docs/foo-post.md"), "should show foo's article:\n{stdout}");
    assert!(!stdout.contains("bar-post"), "should not show bar's article:\n{stdout}");
}

#[test]
fn article_list_all_projects_no_articles() {
    let repo = common::setup_repo();
    common::create_project(&repo, "empty-project");

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path())
        .args(["article", "list"])
        .output()
        .expect("command runs");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("No articles found"), "should show empty message:\n{stdout}");
}

#[test]
fn article_list_all_projects_no_projects_at_all() {
    let repo = common::setup_repo();

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path())
        .args(["article", "list"])
        .output()
        .expect("command runs");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("No articles found"), "should show empty message:\n{stdout}");
}

#[test]
fn article_list_pipe_mode_all_projects_no_headers() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    common::write_doc(&repo, "alpha", "post", "# Post\n");

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path())
        .args(["article", "list"])
        .output()
        .expect("command runs");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    // Pipe mode: no ANSI
    assert!(!stdout.contains("\x1b["), "pipe mode should not have ANSI:\n{stdout}");
    // Header row should NOT be present (pipe mode)
    assert!(!stdout.starts_with("PATH"), "pipe mode should not show headers:\n{stdout}");
}

#[test]
fn article_list_all_projects_hyperlinks() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    common::write_doc(&repo, "alpha", "post", "# Post\n");

    // TTY mode with color to enable hyperlinks
    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path())
        .env("MF_FORCE_TTY", "1")
        .args(["article", "list"])
        .output()
        .expect("command runs");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    // TTY mode: header present
    assert!(stdout.contains("PATH"), "TTY mode should show headers:\n{stdout}");
    assert!(stdout.contains("PROJECT"), "TTY mode should show PROJECT column:\n{stdout}");
}

// ── article block rename tests ──────────────────────────────────────────

#[test]
fn article_block_rename_success() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    // Create a directory article with two blocks
    let article_dir = repo.path().join("my-project/docs/my-article");
    std::fs::create_dir_all(&article_dir).unwrap();
    std::fs::write(article_dir.join("01-opening.md"), "# My Article\n\nintro\n").unwrap();
    std::fs::write(article_dir.join("02-notes.md"), "## Notes\n\nbody\n").unwrap();

    let index_yaml = r#"schema_version: '1'
articles:
  - title: 'My Article'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/my-article'
    status: draft
    created_at: '2026-05-08T10:00:00Z'
    updated_at: '2026-05-08T10:00:00Z'
"#;
    common::write_index(&repo, "my-project", index_yaml);

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "article",
            "block",
            "rename",
            "docs/my-article",
            "02-notes",
            "thoughts",
            "--project",
            "my-project",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("renamed block:"), "stdout: {stdout}");

    assert!(!article_dir.join("02-notes.md").exists(), "old block should be gone");
    assert!(article_dir.join("02-thoughts.md").exists(), "new block should exist");
    let content = std::fs::read_to_string(article_dir.join("02-thoughts.md")).unwrap();
    assert_eq!(content, "## Notes\n\nbody\n");
}

#[test]
fn article_block_rename_dry_run() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let article_dir = repo.path().join("my-project/docs/my-article");
    std::fs::create_dir_all(&article_dir).unwrap();
    std::fs::write(article_dir.join("01-opening.md"), "# Title\n\nintro\n").unwrap();
    std::fs::write(article_dir.join("02-notes.md"), "## Notes\n\nbody\n").unwrap();

    let index_yaml = r#"schema_version: '1'
articles:
  - title: 'My Article'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/my-article'
    status: draft
    created_at: '2026-05-08T10:00:00Z'
    updated_at: '2026-05-08T10:00:00Z'
"#;
    common::write_index(&repo, "my-project", index_yaml);

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "article",
            "block",
            "rename",
            "docs/my-article",
            "02-notes",
            "thoughts",
            "--project",
            "my-project",
            "--dry-run",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[dry-run] would rename block:"), "stdout: {stdout}");

    assert!(article_dir.join("02-notes.md").exists(), "old block should still exist");
    assert!(!article_dir.join("02-thoughts.md").exists(), "new block should not exist");
}

#[test]
fn article_block_rename_not_directory_article_errors() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    std::fs::create_dir_all(repo.path().join("my-project/docs")).unwrap();
    std::fs::write(repo.path().join("my-project/docs/my-article.md"), "# Title\n\ncontent\n").unwrap();

    let index_yaml = r#"schema_version: '1'
articles:
  - title: 'My Article'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/my-article.md'
    status: draft
    created_at: '2026-05-08T10:00:00Z'
    updated_at: '2026-05-08T10:00:00Z'
"#;
    common::write_index(&repo, "my-project", index_yaml);

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "article",
            "block",
            "rename",
            "docs/my-article.md",
            "02-notes",
            "new-slug",
            "--project",
            "my-project",
        ])
        .output()
        .unwrap();

    assert_ne!(output.status.code(), Some(0), "should fail for non-directory article");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stderr.contains("not a directory article") || stdout.contains("not a directory article"),
        "stderr: {stderr}, stdout: {stdout}"
    );
}

#[test]
fn article_block_rename_json_envelope() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let article_dir = repo.path().join("my-project/docs/my-article");
    std::fs::create_dir_all(&article_dir).unwrap();
    std::fs::write(article_dir.join("01-opening.md"), "# Title\n\nintro\n").unwrap();
    std::fs::write(article_dir.join("02-notes.md"), "## Notes\n\nbody\n").unwrap();

    let index_yaml = r#"schema_version: '1'
articles:
  - title: 'My Article'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/my-article'
    status: draft
    created_at: '2026-05-08T10:00:00Z'
    updated_at: '2026-05-08T10:00:00Z'
"#;
    common::write_index(&repo, "my-project", index_yaml);

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "article",
            "block",
            "rename",
            "docs/my-article",
            "02-notes",
            "thoughts",
            "--project",
            "my-project",
            "--json",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    assert_eq!(v["data"]["kind"], "block");
    assert_eq!(v["data"]["new_identity"], "docs/my-article/02-thoughts.md");
    assert_eq!(v["data"]["old_identity"], "docs/my-article/02-notes.md");
}

#[test]
fn article_block_rename_by_title_finds_article() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let article_dir = repo.path().join("my-project/docs/my-article");
    std::fs::create_dir_all(&article_dir).unwrap();
    std::fs::write(article_dir.join("01-opening.md"), "# Title\n\nintro\n").unwrap();
    std::fs::write(article_dir.join("02-notes.md"), "## Notes\n\nbody\n").unwrap();

    let index_yaml = r#"schema_version: '1'
articles:
  - title: 'My Article'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/my-article'
    status: draft
    created_at: '2026-05-08T10:00:00Z'
    updated_at: '2026-05-08T10:00:00Z'
"#;
    common::write_index(&repo, "my-project", index_yaml);

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "article",
            "block",
            "rename",
            "My Article",
            "02-notes",
            "thoughts",
            "--project",
            "my-project",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert!(article_dir.join("02-thoughts.md").exists());
}
