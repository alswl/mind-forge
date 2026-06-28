//! CLI coverage for the default `rules` ASR post-correction engine (spec 055).
//!
//! Backfills tasks T005 (engine/threshold arg contract) and T016 (rules-engine
//! CLI behavior). Asserts SC-001 (glossary homophone `机器仁`→`机器人`), the
//! `engine` finding field, lint read-only / fix dry-run / atomic fix, ASCII
//! pass-through, and the `--engine` / `--ppl-threshold` usage-error contract.

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
    assert_eq!(f["engine"], "rules", "engine field: {f}");
    assert_eq!(f["original"], "机器仁", "original: {f}");
    assert_eq!(f["correct"], "机器人", "correct: {f}");
    assert_eq!(f["replacement_eligible"], true, "replacement_eligible: {f}");
}

// ── Explicit --engine rules behaves identically to the default ──────────────

#[test]
fn explicit_engine_rules_matches_default() {
    let repo = setup_rules_repo();
    write_doc(&repo, "demo", "机器仁开始工作\n");

    let output =
        mf(&repo).args(["term", "lint", "--project", "alpha", "--engine", "rules", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let fs_ = findings(&stdout);
    assert_eq!(fs_.len(), 1, "explicit rules yields the same finding; stdout: {stdout}");
    assert_eq!(fs_[0]["engine"], "rules");
    assert_eq!(fs_[0]["correct"], "机器人");
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

// ── FR-001: invalid engine is a usage error (exit 2) ────────────────────────

#[test]
fn invalid_engine_exits_2() {
    let repo = setup_rules_repo();
    write_doc(&repo, "demo", "机器仁开始工作\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha", "--engine", "bogus"]).output().unwrap();
    assert_eq!(output.status.code(), Some(2), "invalid engine exits 2");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("invalid engine"), "stderr explains the error: {stderr}");
}

// ── FR-009: --ppl-threshold without --engine lm is a usage error ────────────

#[test]
fn ppl_threshold_without_lm_exits_2() {
    let repo = setup_rules_repo();
    write_doc(&repo, "demo", "机器仁开始工作\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha", "--ppl-threshold", "0.3"]).output().unwrap();
    assert_eq!(output.status.code(), Some(2), "threshold without lm exits 2");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--ppl-threshold"), "stderr names the offending flag: {stderr}");
}

// ── FR-009/FR-L5: out-of-range threshold is a usage error ───────────────────

#[test]
fn ppl_threshold_out_of_range_exits_2() {
    let repo = setup_rules_repo();
    write_doc(&repo, "demo", "机器仁开始工作\n");

    let output = mf(&repo)
        .args(["term", "lint", "--project", "alpha", "--engine", "lm", "--ppl-threshold", "1.5"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2), "out-of-range threshold exits 2");
}

// ── LM engine (spec 055): heuristic mode with jieba candidates ────────────────

#[test]
fn lm_engine_finds_homophone_heuristic() {
    let repo = setup_rules_repo();
    write_doc(&repo, "demo", "机器仁开始工作\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha", "--engine", "lm", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(output.status.code(), Some(1), "lm lint with finding exits 1; stdout: {stdout}");

    let fs_ = findings(&stdout);
    assert!(!fs_.is_empty(), "lm engine should find homophone; stdout: {stdout}");
    let f = &fs_[0];
    assert_eq!(f["engine"], "lm", "engine field: {f}");
    assert_eq!(f["original"], "机器仁", "original: {f}");
    assert_eq!(f["correct"], "机器人", "correct: {f}");
    assert_eq!(f["model_version"], "heuristic", "model_version: {f}");
    // PPL fields must be null in heuristic mode.
    assert!(f["ppl_before"].is_null(), "ppl_before must be null: {f}");
    assert!(f["ppl_after"].is_null(), "ppl_after must be null: {f}");
    assert!(f["ppl_improvement"].is_null(), "ppl_improvement must be null: {f}");
    assert_eq!(f["replacement_eligible"], true, "replacement_eligible: {f}");
}

#[test]
fn lm_engine_clean_document_no_finding() {
    let repo = setup_rules_repo();
    write_doc(&repo, "demo", "机器人开始工作\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha", "--engine", "lm", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success(), "clean lm lint exits 0: {stdout}");
    assert_eq!(findings(&stdout).len(), 0, "lm engine: no findings for clean doc");
}

#[test]
fn lm_engine_ascii_document_no_finding() {
    let repo = setup_rules_repo();
    write_doc(&repo, "demo", "robot starts working\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha", "--engine", "lm", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success(), "ascii lm lint exits 0: {stdout}");
    assert_eq!(findings(&stdout).len(), 0, "lm engine: no findings for ASCII doc");
}

#[test]
fn lm_engine_with_valid_threshold_works() {
    let repo = setup_rules_repo();
    write_doc(&repo, "demo", "机器仁开始工作\n");

    let output = mf(&repo)
        .args(["term", "lint", "--project", "alpha", "--engine", "lm", "--ppl-threshold", "0.10", "--json"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    // 0.10 is a valid threshold — exit code depends on whether findings exist.
    let fs_ = findings(&stdout);
    assert!(!fs_.is_empty(), "lm with valid threshold produces findings; stdout: {stdout}");
    assert_eq!(fs_[0]["engine"], "lm");
}

#[test]
fn lm_and_rules_engines_produce_different_engine_tags() {
    let repo = setup_rules_repo();
    write_doc(&repo, "demo", "机器仁开始工作\n");

    let rules_out =
        mf(&repo).args(["term", "lint", "--project", "alpha", "--engine", "rules", "--json"]).output().unwrap();
    let lm_out = mf(&repo).args(["term", "lint", "--project", "alpha", "--engine", "lm", "--json"]).output().unwrap();

    let rules_stdout = String::from_utf8(rules_out.stdout).unwrap();
    let lm_stdout = String::from_utf8(lm_out.stdout).unwrap();

    let rules_fs = findings(&rules_stdout);
    let lm_fs = findings(&lm_stdout);

    assert!(!rules_fs.is_empty(), "rules engine finds homophone");
    assert!(!lm_fs.is_empty(), "lm engine finds homophone");
    assert_eq!(rules_fs[0]["engine"], "rules");
    assert_eq!(lm_fs[0]["engine"], "lm");
    assert_eq!(lm_fs[0]["model_version"], "heuristic");
    // Rules engine has no model_version.
    assert!(rules_fs[0]["model_version"].is_null());
}
