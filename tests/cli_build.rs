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
        .args(["article", "new", "blog", "Test Article"])
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
        .args(["article", "new", "blog", "Configurable Output"])
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
        "schema: '1'\nlayout:\n  docs: notes\nbuild:\n  output_dir: outputs\n  format: md\n",
    );

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "new", "blog", "Custom Docs"])
        .assert()
        .success();

    assert!(repo.path().join("my-project/notes/custom-docs.md").exists());
    assert!(!repo.path().join("my-project/docs/custom-docs.md").exists());

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
        .args(["article", "new", "blog", "Dry Run Test"])
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
    common::write_index(
        &repo,
        "my-project",
        r#"schema: '1'
articles:
  stale-entry:
    title: Review
    source_path: docs/stale-entry
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
