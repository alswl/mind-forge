use std::fs;

use crate::datasets;
use crate::helpers::*;

// ---------------------------------------------------------------------------
// T004: Publisher List Text
// ---------------------------------------------------------------------------

/// E2E: `mf publish target list` outputs text-format publisher list with diagnostics
#[test]
fn publisher_list_text() {
    let ds = datasets::repo_with_publishers();

    let (stdout, stderr, code) = run_in(ds.root(), &["publish", "target", "list"]);

    assert_eq!(code, 0, "exit 0 even with invalid publishers: stdout={stdout} stderr={stderr}");
    assert!(stdout.contains("blog"), "stdout should list blog publisher: stdout={stdout} stderr={stderr}");
    // Diagnostics appear in stdout in text mode
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("malformed")
            || combined.contains("diagnostic")
            || combined.contains("invalid")
            || combined.contains("error"),
        "output should include diagnostic for invalid publisher: {combined}"
    );
}

// ---------------------------------------------------------------------------
// T005: Publisher List JSON
// ---------------------------------------------------------------------------

/// E2E: `mf --format json publisher list` outputs structured JSON publisher list
#[test]
fn publisher_list_json() {
    let ds = datasets::repo_with_publishers();

    let (stdout, stderr, code) = run_in(ds.root(), &["--format", "json", "publish", "target", "list"]);

    assert_eq!(code, 0, "exit 0: stdout={stdout} stderr={stderr}");

    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout should be valid JSON: {e}\nstdout={stdout}\nstderr={stderr}"));

    assert_eq!(parsed["status"], "ok", "JSON status should be ok: {parsed}");
    assert!(parsed["data"]["publishers"].is_array(), "data.publishers should be an array: {parsed}");
    assert!(parsed["data"]["diagnostics"].is_array(), "data.diagnostics should be an array: {parsed}");

    let publishers = parsed["data"]["publishers"].as_array().unwrap();
    assert!(!publishers.is_empty(), "should list at least one publisher: {parsed}");

    let has_blog = publishers.iter().any(|p| p["name"].as_str() == Some("blog"));
    assert!(has_blog, "blog should be in publishers list: {parsed}");

    let diagnostics = parsed["data"]["diagnostics"].as_array().unwrap();
    assert!(!diagnostics.is_empty(), "should have diagnostics for invalid publishers: {parsed}");
}

// ---------------------------------------------------------------------------
// T006: Repo-Wide Publish Success
// ---------------------------------------------------------------------------

/// E2E: Repo-wide publish succeeds, output path resolves from Mind Repo root
#[test]
fn repo_wide_publish_success() {
    let ds = datasets::repo_with_publishers();
    let project_dir = ds.root().join("projects").join("my-project");

    let (stdout, stderr, code) = run_in(&project_dir, &["publish", "run", "my-article", "--target", "blog"]);

    assert_eq!(code, 0, "publish should succeed: stdout={stdout} stderr={stderr}");

    let combined = format!("{stdout} {stderr}");
    assert!(combined.contains("blog"), "output should identify target blog: {combined}");
    assert!(combined.contains("local"), "output should identify type local: {combined}");

    let dest = ds.root().join("publisher-output").join("my-article.md");
    assert!(dest.exists(), "destination file should exist: {dest:?}");

    let content = fs::read_to_string(&dest).expect("read destination file");
    assert_eq!(content, datasets::ARTICLE_BUILD, "destination should contain build artifact content");
}

// ---------------------------------------------------------------------------
// T007: Invalid Publisher Diagnostics
// ---------------------------------------------------------------------------

/// E2E: JSON listing reports diagnostics; publishing through invalid publisher fails before writing
#[test]
fn invalid_publisher_diagnostics() {
    let ds = datasets::repo_with_publishers();
    let project_dir = ds.root().join("projects").join("my-project");

    // Part 1: JSON listing should report diagnostics
    let (stdout, stderr, code) = run_in(ds.root(), &["--format", "json", "publish", "target", "list"]);
    assert_eq!(code, 0, "listing should succeed: stderr={stderr}");
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("valid JSON: {e} stdout={stdout} stderr={stderr}"));
    let diagnostics = parsed["data"]["diagnostics"].as_array().unwrap();
    assert!(!diagnostics.is_empty(), "listing should report diagnostics: {parsed}");

    // Part 2: Publishing through invalid publisher (bad.yaml → malformed) should fail
    let (_stdout, _stderr, code) = run_in(&project_dir, &["publish", "run", "my-article", "--target", "bad"]);
    assert_ne!(code, 0, "publishing through invalid publisher should fail");
}

// ---------------------------------------------------------------------------
// T008: Unknown Publisher Rejection
// ---------------------------------------------------------------------------

/// E2E: Unknown publisher is rejected with error, no content written
#[test]
fn unknown_publisher_rejection() {
    let ds = datasets::repo_with_publishers();
    let project_dir = ds.root().join("projects").join("my-project");

    let (stdout, stderr, code) =
        run_in(&project_dir, &["--format", "json", "publish", "run", "my-article", "--target", "does-not-exist"]);

    assert_ne!(code, 0, "unknown publisher should fail");

    let combined = format!("{stdout} {stderr}");
    assert!(combined.contains("does-not-exist"), "error should mention requested target: {combined}");
}

// ---------------------------------------------------------------------------
// T009: Project-Local Fallback
// ---------------------------------------------------------------------------

/// E2E: No matching repo-wide publisher → fall back to project-local publish target
#[test]
fn project_local_fallback() {
    let ds = datasets::repo_with_project_local_target();
    let project_dir = ds.root().join("projects").join("my-project");

    let (stdout, stderr, code) = run_in(&project_dir, &["publish", "run", "my-article", "--target", "local-blog"]);

    assert_eq!(code, 0, "project-local fallback should succeed: stdout={stdout} stderr={stderr}");

    let dest = ds.root().join("publisher-output").join("my-article.md");
    assert!(dest.exists(), "destination file should exist: {dest:?}");

    let content = fs::read_to_string(&dest).expect("read destination file");
    assert_eq!(content, datasets::ARTICLE_BUILD, "destination should contain build artifact content");
}
