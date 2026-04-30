use assert_cmd::Command;
use predicates::str;

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
    let output_path = repo.path().join("my-project").join("_build").join("test-article.md");
    assert!(output_path.exists(), "output file should exist at {output_path:?}");
    let content = std::fs::read_to_string(&output_path).unwrap();
    assert!(content.contains("Test Article"), "output should contain article title");
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
    let output_path = repo.path().join("my-project").join("_build").join("dry-run-test.md");
    assert!(!output_path.exists(), "dry-run should not create output file");
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
