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
        panic!(
            "lint --fix failed (exit {:?})\nstderr: {stderr}\nstdout: {stdout}",
            output.status.code()
        );
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
    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(cwd)
        .args(&all)
        .output()
        .expect("command runs");
    let out = output.stdout.clone();
    let stdout = String::from_utf8(out).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("JSON parse error: {e}\nstdout: {stdout:?}"));
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
        .args(["article", "new", "Indexed Article"])
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

    let (parsed, output) = json_index(&["--project", "my-project"], &repo.path());
    assert_eq!(output.status.code(), Some(0), "should succeed with --project flag");
    assert!(parsed["data"]["articles_count"].as_u64().unwrap_or(0) >= 1);
}
