//! T062-T065 — Argument-shape contracts for the renamed term surface (US6).
//!
//! These tests lock the canonical positional-subject + flag-metadata shape
//! that 046 standardised on:
//!   * `mf term new <NAME>` — positional; the removed `--term` flag is rejected.
//!   * JSON envelope `data` payloads use `term` (canonical) and `alias`
//!     (variant) — never `original` / `correct` / `name`.

use assert_cmd::Command;
use std::fs;

mod common;

fn mf(repo: &common::TempDir) -> Command {
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.args(["--root", repo.path().to_str().unwrap()]);
    cmd
}

fn setup() -> common::TempDir {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    fs::create_dir_all(project.join("docs")).unwrap();
    common::write_index(&repo, "alpha", "schema_version: '1'\n");
    repo
}

// ── T062: positional <NAME> ────────────────────────────────────────────────

#[test]
fn term_new_accepts_positional_subject() {
    let repo = setup();
    let output = mf(&repo).args(["--project", "alpha", "term", "new", "FooBar"]).output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    // The success message names the term verbatim.
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("FooBar"), "stdout must echo the positional value: {stdout}");

    // No deprecation WARN must fire for the positional form.
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(!stderr.contains("--term flag is deprecated"), "positional form must not warn: {stderr}");
}

#[test]
fn term_new_without_subject_errors_with_usage_hint() {
    let repo = setup();
    let output = mf(&repo).args(["--project", "alpha", "term", "new"]).output().unwrap();
    assert!(!output.status.success(), "must fail when subject is missing");
    assert_eq!(output.status.code(), Some(2), "usage errors exit 2");

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("term name required") || stderr.to_lowercase().contains("required"), "stderr: {stderr}");
}

// ── T063: removed --term flag is absent and rejected ──────────────────────

#[test]
fn term_new_help_omits_removed_term_flag() {
    let repo = setup();
    let output = mf(&repo).args(["term", "new", "--help"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    // Positional <TERM> is visible; the removed flag is not.
    assert!(stdout.contains("[TERM]") || stdout.contains("<TERM>"), "positional argument must show in help: {stdout}");
    assert!(!stdout.contains("--term <"), "deprecated --term flag must be hidden: {stdout}");
}

#[test]
fn term_new_rejects_removed_term_flag() {
    let repo = setup();
    let output = mf(&repo).args(["--project", "alpha", "term", "new", "--term", "FooBar"]).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("unexpected argument '--term'"), "stderr: {stderr}");
}

// ── T064: JSON envelope field names — `term` / `alias`, never `original` / `correct` ──

fn run_json(repo: &common::TempDir, args: &[&str]) -> serde_json::Value {
    let mut cmd = mf(repo);
    cmd.args(args).arg("--format").arg("json");
    let output = cmd.output().unwrap();
    assert!(
        output.status.success(),
        "command failed: args={args:?} stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!("expected JSON envelope, got: {} (err: {e})", String::from_utf8_lossy(&output.stdout))
    })
}

#[test]
fn json_envelope_uses_canonical_term_field_name_across_term_commands() {
    let repo = setup();

    // term new — `data.term`
    let v = run_json(&repo, &["--project", "alpha", "term", "new", "Canonical", "--alias", "alt"]);
    let data = &v["data"];
    assert!(data["term"].is_string(), "term new data must have `term`: {data}");
    assert!(data["added_aliases"].is_array(), "term new data must have `added_aliases`: {data}");
    assert!(data.get("original").is_none(), "must not expose legacy `original` field: {data}");
    assert!(data.get("name").is_none(), "must not expose `name` field: {data}");

    // term show — `data.term`
    let v = run_json(&repo, &["--project", "alpha", "term", "show", "Canonical"]);
    let data = &v["data"];
    assert_eq!(data["term"], "Canonical", "term show must use `term` key: {data}");
    assert!(data["aliases"].is_array(), "term show must use plural `aliases`: {data}");
    assert!(data.get("name").is_none() && data.get("variant").is_none(), "no `name`/`variant` keys: {data}");

    // term update — verb envelope keeps `identity` but inner `data.identity` should hold the canonical term
    let v = run_json(&repo, &["--project", "alpha", "term", "update", "Canonical", "--tag", "infra"]);
    let data = &v["data"];
    assert_eq!(data["identity"], "Canonical", "term update identity must be the canonical term: {data}");

    // term list — items use `term` not `name`
    let v = run_json(&repo, &["--project", "alpha", "term", "list"]);
    let terms = v["data"]["terms"].as_array().unwrap();
    assert!(!terms.is_empty(), "list should include the seeded term");
    for t in terms {
        assert!(t["term"].is_string(), "list item must use `term`: {t}");
        assert!(t.get("name").is_none(), "list item must not use `name`: {t}");
        assert!(t["aliases"].is_array(), "list item aliases must be an array: {t}");
    }
}
