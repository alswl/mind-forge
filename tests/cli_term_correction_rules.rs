//! CLI coverage for the `rules` ASR post-correction engine (spec 055/058).
//!
//! Asserts SC-001 (glossary homophone `机器仁`→`机器人`), lint read-only /
//! fix dry-run / atomic fix, ASCII pass-through, and that the removed
//! `--engine` / `--ppl-threshold` flags are now rejected (spec 058).

use assert_cmd::Command;
use serde_json::Value;
use std::fs;

mod common;

/// Repo with project `alpha`, a `docs/` dir, and glossary term `机器人`
/// (no declared correction — the rules engine must infer the homophone).
fn setup_rules_repo() -> common::TempDir {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    fs::create_dir_all(repo.path().join("alpha").join("docs")).unwrap();
    let index = r#"schema_version: '1'
terms:
  - term: 机器人
    definition: robot
    aliases: []
    tags: []
    corrections: []
"#;
    common::write_index(&repo, "alpha", index);
    repo
}

fn write_doc(repo: &common::TempDir, name: &str, content: &str) {
    fs::write(repo.path().join("alpha").join("docs").join(format!("{name}.md")), content).unwrap();
}

fn read_doc(repo: &common::TempDir, name: &str) -> String {
    fs::read_to_string(repo.path().join("alpha").join("docs").join(format!("{name}.md"))).unwrap()
}

fn mf(repo: &common::TempDir) -> Command {
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.args(["--root", repo.path().to_str().unwrap()]);
    cmd
}

fn findings(stdout: &str) -> Vec<Value> {
    let v: Value = serde_json::from_str(stdout).expect("valid JSON envelope");
    v["data"]["findings"].as_array().cloned().unwrap_or_default()
}

// ── SC-001: default lint reports a rules homophone finding ──────────────────

#[test]
fn default_lint_reports_rules_homophone_finding() {
    let repo = setup_rules_repo();
    write_doc(&repo, "demo", "机器仁开始工作\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    // A finding means a non-zero (issue) exit.
    assert_eq!(output.status.code(), Some(1), "lint with finding exits 1; stdout: {stdout}");

    let fs_ = findings(&stdout);
    assert_eq!(fs_.len(), 1, "exactly one finding; stdout: {stdout}");
    let f = &fs_[0];
    assert_eq!(f["original"], "机器仁", "original: {f}");
    assert_eq!(f["correct"], "机器人", "correct: {f}");
    assert_eq!(f["replacement_eligible"], true, "replacement_eligible: {f}");
}

// ── lint never writes ───────────────────────────────────────────────────────

#[test]
fn lint_is_read_only() {
    let repo = setup_rules_repo();
    write_doc(&repo, "demo", "机器仁开始工作\n");

    mf(&repo).args(["term", "lint", "--project", "alpha"]).output().unwrap();
    assert_eq!(read_doc(&repo, "demo"), "机器仁开始工作\n", "lint must not modify the document");
}

// ── fix --dry-run reports the edit but writes nothing ───────────────────────

#[test]
fn fix_dry_run_writes_nothing() {
    let repo = setup_rules_repo();
    write_doc(&repo, "demo", "机器仁开始工作\n");

    let output = mf(&repo).args(["term", "fix", "--project", "alpha", "--dry-run", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["data"]["dry_run"], true, "dry_run flag set: {stdout}");
    assert_eq!(v["data"]["would_fix_count"], 1, "one edit would apply: {stdout}");
    assert_eq!(v["data"]["modified_files"].as_array().unwrap().len(), 0, "no files modified: {stdout}");
    assert_eq!(read_doc(&repo, "demo"), "机器仁开始工作\n", "dry-run must not write");
}

// ── SC-001 fix path: confirmed fix atomically rewrites the document ─────────

#[test]
fn fix_yes_rewrites_homophone() {
    let repo = setup_rules_repo();
    write_doc(&repo, "demo", "机器仁开始工作\n");

    let output = mf(&repo).args(["term", "fix", "--project", "alpha", "--yes"]).output().unwrap();
    assert!(output.status.success(), "fix --yes succeeds: {:?}", output);
    assert_eq!(read_doc(&repo, "demo"), "机器人开始工作\n", "fix must rewrite 机器仁→机器人");
}

// ── A clean document yields no finding and exit 0 ───────────────────────────

#[test]
fn clean_document_has_no_finding() {
    let repo = setup_rules_repo();
    write_doc(&repo, "demo", "机器人开始工作\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success(), "clean doc exits 0: {stdout}");
    assert_eq!(findings(&stdout).len(), 0, "no findings for clean doc: {stdout}");
}

// ── FR-008: ASCII-only spans pass through unchanged ─────────────────────────

#[test]
fn ascii_only_span_unchanged() {
    let repo = setup_rules_repo();
    write_doc(&repo, "demo", "robot starts working\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success(), "ASCII doc exits 0: {stdout}");
    assert_eq!(findings(&stdout).len(), 0, "ASCII-only span must not generate a finding: {stdout}");
}

// ── spec 058: the removed engine flags are rejected as unknown args ──────────

#[test]
fn engine_flag_removed_exits_2() {
    let repo = setup_rules_repo();
    write_doc(&repo, "demo", "机器仁开始工作\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha", "--engine", "rules"]).output().unwrap();
    assert_eq!(output.status.code(), Some(2), "--engine is no longer a recognized flag");
}

#[test]
fn ppl_threshold_flag_removed_exits_2() {
    let repo = setup_rules_repo();
    write_doc(&repo, "demo", "机器仁开始工作\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha", "--ppl-threshold", "0.3"]).output().unwrap();
    assert_eq!(output.status.code(), Some(2), "--ppl-threshold is no longer a recognized flag");
}
