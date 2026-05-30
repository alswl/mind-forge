use assert_cmd::Command;
use tempfile::TempDir;

mod common;

fn seed_terms(repo: &TempDir, project_name: &str) {
    common::write_term_index(
        repo,
        project_name,
        "term: Mind Repo\n    definition: A repository of minds\n    corrections:\n      - original: mindrepo\n        correct: Mind Repo",
    );
}

fn setup_with_terms() -> TempDir {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    seed_terms(&repo, "alpha");
    repo
}

// ---------------------------------------------------------------------------
// 1. remove_term_success — term removed from index
// ---------------------------------------------------------------------------

#[test]
fn remove_term_success() {
    let repo = setup_with_terms();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "remove", "Mind Repo", "--project", "alpha", "--yes"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let index_content = std::fs::read_to_string(repo.path().join("alpha").join("mind-index.yaml")).unwrap();
    assert!(!index_content.contains("Mind Repo"), "term should be removed, got: {index_content}");
}

// ---------------------------------------------------------------------------
// 2. remove_term_not_found — unknown term → usage error
// ---------------------------------------------------------------------------

#[test]
fn remove_term_not_found() {
    let repo = setup_with_terms();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "remove", "Nonexistent", "--project", "alpha", "--yes"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not found"), "stderr: {stderr}");
}

// ---------------------------------------------------------------------------
// 3. remove_term_json_envelope — JSON output with verb-style envelope
// ---------------------------------------------------------------------------

#[test]
fn remove_term_json_envelope() {
    let repo = setup_with_terms();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "term",
            "remove",
            "Mind Repo",
            "--project",
            "alpha",
            "--yes",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    assert_eq!(v["data"]["kind"], "term");
    assert_eq!(v["data"]["identity"], "Mind Repo");
    assert_eq!(v["data"]["removed"], true);
    assert_eq!(v["data"]["dry_run"], false);
}

// ---------------------------------------------------------------------------
// 4. remove_term_dry_run — dry run does not mutate
// ---------------------------------------------------------------------------

#[test]
fn remove_term_dry_run() {
    let repo = setup_with_terms();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "term",
            "remove",
            "Mind Repo",
            "--project",
            "alpha",
            "--dry-run",
            "--yes",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    // Term should still exist in index
    let index_content = std::fs::read_to_string(repo.path().join("alpha").join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("Mind Repo"), "term should still exist after dry run");
}

// ---------------------------------------------------------------------------
// 5. remove_term_rm_alias — `rm` alias works
// ---------------------------------------------------------------------------

#[test]
fn remove_term_rm_alias() {
    let repo = setup_with_terms();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "rm", "Mind Repo", "--project", "alpha", "--yes"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let index_content = std::fs::read_to_string(repo.path().join("alpha").join("mind-index.yaml")).unwrap();
    assert!(!index_content.contains("Mind Repo"), "term should be removed via `rm` alias");
}
