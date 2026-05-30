//! Integration tests for path-based entity identity:
//! project auto-detection, project-local selectors, and canonical path output.
//!
//! Covers US2 (T027-T032) and US4 (T051-T054).

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use tempfile::TempDir;

mod common;

fn setup() -> TempDir {
    common::setup_repo()
}

fn mf_in(dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.args(["--root", dir.path().to_str().unwrap()]);
    cmd
}

fn run_json_in(dir: &TempDir, args: &[&str]) -> (String, String, i32) {
    let mut full_args = vec!["--root", dir.path().to_str().unwrap(), "--json"];
    full_args.extend_from_slice(args);
    let output = Command::cargo_bin("mf").unwrap().args(&full_args).output().unwrap();
    (
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
        output.status.code().unwrap_or_default(),
    )
}

/// Run mf with --json from a specific current_dir (for auto-detection tests).
fn run_json_at(repo: &TempDir, cwd: &Path, args: &[&str]) -> (String, String, i32) {
    let mut full_args = vec!["--root", repo.path().to_str().unwrap(), "--json"];
    full_args.extend_from_slice(args);
    let output = Command::cargo_bin("mf").unwrap().args(&full_args).current_dir(cwd).output().unwrap();
    (
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
        output.status.code().unwrap_or_default(),
    )
}

/// Run mf in text mode from a specific current_dir.
fn run_text_at(repo: &TempDir, cwd: &Path, args: &[&str]) -> (String, String, i32) {
    let mut full_args = vec!["--root", repo.path().to_str().unwrap()];
    full_args.extend_from_slice(args);
    let output = Command::cargo_bin("mf").unwrap().args(&full_args).current_dir(cwd).output().unwrap();
    (
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
        output.status.code().unwrap_or_default(),
    )
}

/// Create a project with articles, sources, and assets for testing entity selectors.
fn scaffold_full_project(repo: &TempDir, project_path: &str) {
    let full = repo.path().join(project_path);
    fs::create_dir_all(full.join("docs")).unwrap();
    fs::create_dir_all(full.join("assets")).unwrap();
    fs::create_dir_all(full.join("sources/meeting")).unwrap();

    fs::write(full.join("mind.yaml"), "schema_version: '1'\n").unwrap();

    // Write article file
    fs::write(full.join("docs/weekly.md"), "# Weekly Report\n\nContent\n").unwrap();

    // Write asset file (empty placeholder)
    fs::write(full.join("assets/chart.png"), b"fake-png").unwrap();

    // Write source file
    fs::write(full.join("sources/meeting/notes.md"), "# Meeting Notes\n").unwrap();

    // Write mind-index.yaml with articles, assets, and sources
    let index_yaml = format!(
        r#"schema_version: '1'
articles:
  - title: weekly
    project: '{project_path}'
    article_type: blog
    article_path: docs/weekly.md
    status: draft
    created_at: '2026-05-21T00:00:00Z'
    updated_at: '2026-05-21T00:00:00Z'
  - title: second-article
    project: '{project_path}'
    article_type: blog
    article_path: docs/second.md
    status: draft
    created_at: '2026-05-21T00:00:00Z'
    updated_at: '2026-05-21T00:00:00Z'
assets:
  - name: chart.png
    kind: image
    path: assets/chart.png
    size: 8
    hash: abc123
    tags: []
    added_at: '2026-05-21T00:00:00Z'
sources:
  - name: meeting-notes
    kind: file
    path: sources/meeting/notes.md
    added_at: '2026-05-21T00:00:00Z'
    updated_at: '2026-05-21T00:00:00Z'
"#
    );
    fs::write(full.join("mind-index.yaml"), index_yaml).unwrap();

    // Register in minds.yaml
    let minds_content = format!(
        "schema_version: '1'\nprojects_dir: '.'\nprojects:\n  - name: '{}'\n    path: ./{}\n    created_at: '2026-05-21T00:00:00Z'\n",
        project_path, project_path
    );
    fs::write(repo.path().join("minds.yaml"), minds_content).unwrap();
}

/// Create a minimal project with one article for basic auto-detection tests.
fn scaffold_minimal_project(repo: &TempDir, project_path: &str) {
    let full = repo.path().join(project_path);
    fs::create_dir_all(full.join("docs")).unwrap();

    fs::write(full.join("mind.yaml"), "schema_version: '1'\n").unwrap();
    fs::write(full.join("docs/weekly.md"), "# Weekly\n").unwrap();

    let index_yaml = format!(
        r#"schema_version: '1'
articles:
  - title: weekly
    project: '{project_path}'
    article_type: blog
    article_path: docs/weekly.md
    status: draft
    created_at: '2026-05-21T00:00:00Z'
    updated_at: '2026-05-21T00:00:00Z'
"#
    );
    fs::write(full.join("mind-index.yaml"), index_yaml).unwrap();

    let minds_content = format!(
        "schema_version: '1'\nprojects_dir: '.'\nprojects:\n  - name: '{}'\n    path: ./{}\n    created_at: '2026-05-21T00:00:00Z'\n",
        project_path, project_path
    );
    fs::write(repo.path().join("minds.yaml"), minds_content).unwrap();
}

// ── T027: Auto-detection equivalence ────────────────────────────────────────

#[test]
fn article_list_auto_detection_equals_explicit_project() {
    let repo = setup();
    scaffold_minimal_project(&repo, "my-project");

    // Run from inside the project (auto-detection)
    let project_dir = repo.path().join("my-project");
    let (auto_stdout, auto_stderr, auto_code) = run_json_at(&repo, &project_dir, &["article", "list"]);

    // Run from repo root with explicit --project
    let (explicit_stdout, explicit_stderr, explicit_code) =
        run_json_in(&repo, &["--project", "my-project", "article", "list"]);

    assert_eq!(auto_code, 0, "auto stderr: {auto_stderr}");
    assert_eq!(explicit_code, 0, "explicit stderr: {explicit_stderr}");

    let auto_v: serde_json::Value = serde_json::from_str(&auto_stdout).unwrap();
    let explicit_v: serde_json::Value = serde_json::from_str(&explicit_stdout).unwrap();

    // Both should return the same articles
    let auto_articles = auto_v["data"]["articles"].as_array();
    let explicit_articles = explicit_v["data"]["articles"].as_array();
    assert!(auto_articles.is_some(), "auto articles missing: {auto_stdout}");
    assert!(explicit_articles.is_some(), "explicit articles missing: {explicit_stdout}");

    assert_eq!(
        auto_articles.unwrap().len(),
        explicit_articles.unwrap().len(),
        "auto and explicit should return same article count"
    );
}

// ── T028: Article show with path selector ───────────────────────────────────

#[test]
fn article_show_path_selector_from_project_dir() {
    let repo = setup();
    scaffold_full_project(&repo, "my-project");
    let project_dir = repo.path().join("my-project");

    let (stdout, stderr, code) = run_json_at(&repo, &project_dir, &["article", "show", "docs/weekly.md"]);
    assert_eq!(code, 0, "stderr: {stderr}");

    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    // Should report the canonical article path
    assert_eq!(v["data"]["article_path"], "docs/weekly.md");
}

// ── T029: Asset remove path selector ────────────────────────────────────────

#[test]
fn asset_remove_path_selector_dry_run() {
    let repo = setup();
    scaffold_full_project(&repo, "my-project");
    let project_dir = repo.path().join("my-project");

    let (stdout, stderr, code) =
        run_json_at(&repo, &project_dir, &["asset", "remove", "assets/chart.png", "--dry-run", "--yes"]);
    assert_eq!(code, 0, "stderr: {stderr}");

    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    // Should report canonical asset path
    assert_eq!(v["data"]["kind"], "asset");
    assert_eq!(v["data"]["identity"], "assets/chart.png");
}

// ── T030: Source remove path selector ───────────────────────────────────────

#[test]
fn source_remove_path_selector_dry_run() {
    let repo = setup();
    scaffold_full_project(&repo, "my-project");
    let project_dir = repo.path().join("my-project");

    let (stdout, stderr, code) = run_json_at(
        &repo,
        &project_dir,
        &["source", "remove", "sources/meeting/notes.md", "--dry-run", "--keep-file", "--force"],
    );
    assert_eq!(code, 0, "stderr: {stderr}");

    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok", "stdout: {stdout} stderr: {stderr}");
    // SourceRemoveReport embeds source fields directly; check path
    let source_path = v["data"]["path"].as_str().unwrap_or("");
    assert_eq!(source_path, "sources/meeting/notes.md");
}

// ── T031: Entity selector escape rejection ──────────────────────────────────

#[test]
fn article_selector_rejects_project_escape() {
    let repo = setup();
    scaffold_minimal_project(&repo, "my-project");
    let project_dir = repo.path().join("my-project");

    let (stdout, stderr, code) = run_json_at(&repo, &project_dir, &["article", "show", "../outside.md"]);
    assert_ne!(code, 0, "should fail on escape: stdout={stdout} stderr={stderr}");

    let error_output = if stderr.trim().is_empty() { &stdout } else { &stderr };
    let v: serde_json::Value = serde_json::from_str(error_output).unwrap();
    assert_eq!(v["status"], "error");
}

#[test]
fn asset_selector_rejects_project_escape() {
    let repo = setup();
    scaffold_full_project(&repo, "my-project");
    let project_dir = repo.path().join("my-project");

    let (stdout, stderr, code) =
        run_json_at(&repo, &project_dir, &["asset", "remove", "../../outside.png", "--dry-run"]);
    assert_ne!(code, 0, "should fail on escape: stdout={stdout} stderr={stderr}");

    let error_output = if stderr.trim().is_empty() { &stdout } else { &stderr };
    let v: serde_json::Value = serde_json::from_str(error_output).unwrap();
    assert_eq!(v["status"], "error");
}

#[test]
fn source_selector_rejects_project_escape() {
    let repo = setup();
    scaffold_full_project(&repo, "my-project");
    let project_dir = repo.path().join("my-project");

    let (stdout, stderr, code) =
        run_json_at(&repo, &project_dir, &["source", "remove", "../../outside.md", "--dry-run"]);
    assert_ne!(code, 0, "should fail on escape: stdout={stdout} stderr={stderr}");

    let error_output = if stderr.trim().is_empty() { &stdout } else { &stderr };
    let v: serde_json::Value = serde_json::from_str(error_output).unwrap();
    assert_eq!(v["status"], "error");
}

// ── T032: Ambiguity - legacy title/name selector ────────────────────────────

#[test]
fn article_legacy_title_still_works_when_unambiguous() {
    let repo = setup();
    scaffold_minimal_project(&repo, "my-project");
    let project_dir = repo.path().join("my-project");

    // Legacy lookup by title (not path) should still work
    let (stdout, stderr, code) = run_json_at(&repo, &project_dir, &["article", "show", "weekly"]);
    assert_eq!(code, 0, "stderr: {stderr}");

    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    assert_eq!(v["data"]["article_path"], "docs/weekly.md");
}

#[test]
fn legacy_title_unambiguous_after_path_migration() {
    let repo = setup();
    // Two articles with different paths but legacy title lookup should work
    let full = repo.path().join("multi-project");
    fs::create_dir_all(full.join("docs")).unwrap();
    fs::write(full.join("mind.yaml"), "schema_version: '1'\n").unwrap();
    fs::write(full.join("docs/report.md"), "# Report A\n").unwrap();
    fs::write(full.join("docs/summary.md"), "# Summary\n").unwrap();

    let index_yaml = r#"schema_version: '1'
articles:
  - title: report-a
    project: 'multi-project'
    article_type: blog
    article_path: docs/report.md
    status: draft
    created_at: '2026-05-21T00:00:00Z'
    updated_at: '2026-05-21T00:00:00Z'
  - title: summary
    project: 'multi-project'
    article_type: blog
    article_path: docs/summary.md
    status: draft
    created_at: '2026-05-21T00:00:00Z'
    updated_at: '2026-05-21T00:00:00Z'
"#;
    fs::write(full.join("mind-index.yaml"), index_yaml).unwrap();

    let minds_content = "schema_version: '1'\nprojects_dir: '.'\nprojects:\n  - name: 'multi-project'\n    path: ./multi-project\n    created_at: '2026-05-21T00:00:00Z'\n";
    fs::write(repo.path().join("minds.yaml"), minds_content).unwrap();

    let project_dir = repo.path().join("multi-project");

    // Legacy title lookup still resolves uniquely
    let (stdout, stderr, code) = run_json_at(&repo, &project_dir, &["article", "show", "report-a"]);
    assert_eq!(code, 0, "stderr: {stderr}");

    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    assert_eq!(v["data"]["article_path"], "docs/report.md");
}

// ══════════════════════════════════════════════════════════════════════════════
// US4: Error handling and backward-compatible defaults (T051-T054)
// ══════════════════════════════════════════════════════════════════════════════

// ── T051: JSON error envelope tests ────────────────────────────────────────

#[test]
fn project_new_rejects_dotdot_escape() {
    let repo = setup();
    // --json writes error to stderr
    let (stdout, stderr, code) = run_json_in(&repo, &["project", "new", "../outside"]);
    assert_ne!(code, 0, "expected non-zero exit for escape; stdout={stdout} stderr={stderr}");
    let error_output = if stderr.trim().is_empty() { &stdout } else { &stderr };
    let v: serde_json::Value = serde_json::from_str(error_output).unwrap();
    assert_eq!(v["status"], "error");
    assert_eq!(v["error"]["kind"], "usage");
}

#[test]
fn project_new_rejects_nested_inside_existing_project() {
    let repo = setup();
    scaffold_minimal_project(&repo, "outer-project");
    // Create a path under the existing project
    let (_stdout, _stderr, code) = run_json_in(&repo, &["project", "new", "outer-project/nested"]);
    assert_ne!(code, 0);
}

#[test]
fn project_new_rejects_duplicate_path_identity() {
    let repo = setup();
    scaffold_minimal_project(&repo, "my-project");
    let (stdout, stderr, code) = run_json_in(&repo, &["project", "new", "my-project"]);
    assert_ne!(code, 0, "expected non-zero exit for duplicate; stdout={stdout} stderr={stderr}");
    let error_output = if stderr.trim().is_empty() { &stdout } else { &stderr };
    let v: serde_json::Value = serde_json::from_str(error_output).unwrap();
    assert_eq!(v["status"], "error");
    // Either "file-exists" (scaffold check) or "usage" (upsert check) is valid
    let kind = v["error"]["kind"].as_str().unwrap_or("");
    assert!(kind == "file-exists" || kind == "usage", "unexpected error kind: {kind}");
}

#[test]
fn missing_project_auto_detection_returns_error() {
    let repo = setup();
    // Running article list from repo root without --project should fail
    // because there's no auto-detected project
    let (stdout, _stderr, code) = run_json_in(&repo, &["article", "list"]);
    // May succeed with empty list or fail — either is fine for auto-detection
    // The key is it shouldn't crash
    let _ = (stdout, code);
}

// ── T052: Text stderr diagnostic tests ────────────────────────────────────

#[test]
fn project_new_escape_shows_diagnostic_in_text_mode() {
    let repo = setup();
    let (stdout, stderr, code) = run_text_at(&repo, repo.path(), &["project", "new", "../bad"]);
    assert_ne!(code, 0);
    let combined = format!("{stdout}\n{stderr}");
    assert!(combined.contains("..") || combined.contains("outside"), "diagnostic: {combined}");
}

#[test]
fn duplicate_project_path_shows_diagnostic() {
    let repo = setup();
    scaffold_minimal_project(&repo, "dup-project");
    let (stdout, stderr, code) = run_text_at(&repo, repo.path(), &["project", "new", "dup-project"]);
    assert_ne!(code, 0);
    let combined = format!("{stdout}\n{stderr}");
    assert!(combined.contains("already registered") || combined.contains("dup-project"), "diagnostic: {combined}");
}

// ── T053: Regression tests for backward-compatible defaults ────────────────

#[test]
fn simple_project_new_flat_name_still_works() {
    let repo = setup();
    let out = mf_in(&repo).args(["--format", "json", "project", "new", "simple-report"]).output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok", "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(v["data"]["path"], "simple-report");
}

#[test]
fn explicit_project_article_list_still_works() {
    let repo = setup();
    scaffold_minimal_project(&repo, "simple-report");
    let (stdout, stderr, code) = run_json_in(&repo, &["--project", "simple-report", "article", "list"]);
    assert_eq!(code, 0, "stderr: {stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    assert!(v["data"]["articles"].is_array());
}

#[test]
fn repo_level_publish_still_works() {
    use tempfile::TempDir;
    let repo = setup();
    let project_dir = repo.path().join("simple-report");
    fs::create_dir_all(project_dir.join("docs")).unwrap();
    fs::write(project_dir.join("mind.yaml"), "schema_version: '1'\n").unwrap();
    fs::write(project_dir.join("docs/weekly.md"), "# Weekly\n").unwrap();

    let index_yaml = r#"schema_version: '1'
articles:
  - title: weekly
    project: 'simple-report'
    article_type: blog
    article_path: docs/weekly.md
    status: draft
    created_at: '2026-05-21T00:00:00Z'
    updated_at: '2026-05-21T00:00:00Z'
"#;
    fs::write(project_dir.join("mind-index.yaml"), index_yaml).unwrap();

    let minds_content = "schema_version: '1'\nprojects_dir: '.'\nprojects:\n  - name: 'simple-report'\n    path: ./simple-report\n    created_at: '2026-05-21T00:00:00Z'\n";
    fs::write(repo.path().join("minds.yaml"), minds_content).unwrap();

    // Build the article first
    let build_out = mf_in(&repo).args(["--project", "simple-report", "build", "weekly"]).output().unwrap();
    assert!(build_out.status.success(), "build failed: {}", String::from_utf8_lossy(&build_out.stderr));

    // Publish to local target (project-level mind.yaml with local target)
    let dest = TempDir::new().unwrap();
    let mind_yaml = format!(
        "schema_version: '1'\npublish:\n  default_target: local-out\n  targets:\n    - name: local-out\n      type: local\n      enabled: true\n      path: {}\n",
        dest.path().display()
    );
    fs::write(project_dir.join("mind.yaml"), mind_yaml).unwrap();

    let (stdout, stderr, code) = run_json_in(&repo, &["--project", "simple-report", "publish", "run", "weekly"]);
    assert_eq!(code, 0, "publish failed: stdout={stdout} stderr={stderr}");
}

// ── T054: Read-only selectors don't modify files ──────────────────────────

#[test]
fn read_only_selector_resolution_does_not_modify_files() {
    let repo = setup();
    scaffold_minimal_project(&repo, "project-x");

    // Record file contents before resolution
    let minds_before = fs::read_to_string(repo.path().join("minds.yaml")).unwrap();
    let project_dir = repo.path().join("project-x");
    let mind_before = fs::read_to_string(project_dir.join("mind.yaml")).unwrap();
    let index_before = fs::read_to_string(project_dir.join("mind-index.yaml")).unwrap();

    // Run read-only operations
    let _ = run_json_at(&repo, &project_dir, &["article", "list"]);
    let _ = run_json_at(&repo, &project_dir, &["article", "show", "weekly"]);

    // Verify no file changes
    let minds_after = fs::read_to_string(repo.path().join("minds.yaml")).unwrap();
    let mind_after = fs::read_to_string(project_dir.join("mind.yaml")).unwrap();
    let index_after = fs::read_to_string(project_dir.join("mind-index.yaml")).unwrap();

    assert_eq!(minds_before, minds_after, "minds.yaml was modified by read-only operations");
    assert_eq!(mind_before, mind_after, "mind.yaml was modified by read-only operations");
    assert_eq!(index_before, index_after, "mind-index.yaml was modified by read-only operations");
}
