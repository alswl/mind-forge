use assert_cmd::Command;

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
    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["build", "test-article"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(0), "build should succeed");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Test Article"), "build output should contain article title");
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

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["build", "dry-run-test", "--dry-run"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("dry_run"));
    assert!(stdout.contains("size_bytes"));
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
    assert_eq!(output.status.code(), Some(2), "should fail for non-existent article");
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
