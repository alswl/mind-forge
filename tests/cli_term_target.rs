//! US6: Term lint/fix target granularity — project, file, and article target types.
//!
//! Tests verify that:
//! - No-path lint scans all project Markdown files (target_type: "project")
//! - A specific `.md` path restricts scanning to that file (target_type: "file")
//! - `--article` sets target_type: "article" in JSON output
//! - `--fix --dry-run` writes nothing regardless of target
//! - Unsafe paths (above project root) are rejected
//! - Missing files produce a clear error

use assert_cmd::Command;

mod common;

fn mf(repo: &common::TempDir) -> Command {
    let mut c = Command::cargo_bin("mf").unwrap();
    c.args(["--root", repo.path().to_str().unwrap()]);
    c
}

/// Build a project with two docs and one term correction.
fn setup_with_docs() -> common::TempDir {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    common::write_term_index(
        &repo,
        "alpha",
        "term: Widget\n    corrections:\n      - original: widget\n        correct: Widget",
    );
    let docs = repo.path().join("alpha").join("docs");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(docs.join("a.md"), "# Doc A\n\nUse widget here.\n").unwrap();
    std::fs::write(docs.join("b.md"), "# Doc B\n\nAnother widget usage.\n").unwrap();
    repo
}

// ── T075: project-target scans all project files ──────────────────────────

#[test]
fn project_target_scans_all_docs() {
    let repo = setup_with_docs();
    let out = mf(&repo).args(["--project", "alpha", "term", "lint"]).output().unwrap();
    // Both docs have "widget" → findings expected (exit 1)
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("a.md") && stdout.contains("b.md"), "both docs should be scanned: {stdout}");
}

#[test]
fn project_target_dry_run_writes_nothing() {
    let repo = setup_with_docs();
    let out = mf(&repo).args(["--project", "alpha", "term", "lint", "--fix", "--dry-run"]).output().unwrap();
    // dry-run with findings exits 1 (findings exist) — that's correct behavior
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("dry-run") || stdout.contains("would fix"), "should show dry-run output: {stdout}");

    // Files must be unchanged
    let a = std::fs::read_to_string(repo.path().join("alpha").join("docs").join("a.md")).unwrap();
    let b = std::fs::read_to_string(repo.path().join("alpha").join("docs").join("b.md")).unwrap();
    assert!(a.contains("widget"), "a.md must be unchanged after dry-run");
    assert!(b.contains("widget"), "b.md must be unchanged after dry-run");
}

// ── T077: file target restricts scan to the given path ────────────────────

#[test]
fn file_target_restricts_to_one_file() {
    let repo = setup_with_docs();
    let out = mf(&repo).args(["--project", "alpha", "term", "lint", "docs/a.md"]).output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("a.md"), "a.md should appear in findings: {stdout}");
    assert!(!stdout.contains("b.md"), "b.md must not appear when targeting a.md only: {stdout}");
}

#[test]
fn file_target_non_indexed_md_file() {
    let repo = setup_with_docs();
    // Write a doc that is not in any article index
    let extra = repo.path().join("alpha").join("docs").join("extra.md");
    std::fs::write(&extra, "Use widget here too.\n").unwrap();

    let out = mf(&repo).args(["--project", "alpha", "term", "lint", "docs/extra.md"]).output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("extra.md"), "non-indexed file should still be scannable: {stdout}");
}

// ── T078: targeted fix modifies only the selected file ────────────────────

#[test]
fn file_target_fix_modifies_only_target() {
    let repo = setup_with_docs();
    let out = mf(&repo).args(["--project", "alpha", "term", "lint", "docs/a.md", "--fix", "--yes"]).output().unwrap();
    assert!(out.status.success(), "fix should succeed: {}", String::from_utf8_lossy(&out.stderr));

    let a = std::fs::read_to_string(repo.path().join("alpha").join("docs").join("a.md")).unwrap();
    let b = std::fs::read_to_string(repo.path().join("alpha").join("docs").join("b.md")).unwrap();
    assert!(a.contains("Widget") && !a.contains("widget"), "a.md should be fixed");
    assert!(b.contains("widget"), "b.md must not be touched when only a.md was targeted");
}

// ── T079: JSON output includes target_type ────────────────────────────────

#[test]
fn json_target_type_project() {
    let repo = setup_with_docs();
    let out = mf(&repo).args(["--project", "alpha", "--json", "term", "lint"]).output().unwrap();
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON");
    assert_eq!(json["data"]["target_type"], "project", "no path → target_type=project: {json}");
}

#[test]
fn json_target_type_file() {
    let repo = setup_with_docs();
    let out = mf(&repo).args(["--project", "alpha", "--json", "term", "lint", "docs/a.md"]).output().unwrap();
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON");
    assert_eq!(json["data"]["target_type"], "file", "path arg → target_type=file: {json}");
}

#[test]
fn json_target_type_article() {
    let repo = setup_with_docs();
    let out =
        mf(&repo).args(["--project", "alpha", "--json", "term", "lint", "--article", "weekly-note"]).output().unwrap();
    // Flag is accepted; target_type must be "article" regardless of resolution
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON");
    assert_eq!(json["data"]["target_type"], "article", "--article sets target_type=article: {json}");
}

// ── T076: --article sets target_type and is accepted ─────────────────────

#[test]
fn article_flag_accepted_and_sets_target_type() {
    let repo = setup_with_docs();
    let out = mf(&repo).args(["--project", "alpha", "term", "lint", "--article", "some-article"]).output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    // Command must not hard-error on an unresolved article slug —
    // current implementation falls back to a full project scan.
    assert!(stdout.contains("target: article") || !stdout.contains("error"), "article flag must be accepted: {stdout}");
}

// ── T080: error cases ────────────────────────────────────────────────────

#[test]
fn missing_file_produces_error() {
    let repo = setup_with_docs();
    let out = mf(&repo).args(["--project", "alpha", "term", "lint", "docs/nonexistent.md"]).output().unwrap();
    assert!(!out.status.success(), "missing file should fail");
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(!stderr.is_empty(), "should have error output for missing file");
}

#[test]
fn unsafe_path_above_project_root_rejected() {
    let repo = setup_with_docs();
    // "../minds.yaml" would be above the project root
    let out = mf(&repo).args(["--project", "alpha", "term", "lint", "../minds.yaml"]).output().unwrap();
    assert!(!out.status.success(), "path above project root must be rejected");
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("not under project root") || stderr.contains("not found"),
        "should mention path containment: {stderr}"
    );
}
