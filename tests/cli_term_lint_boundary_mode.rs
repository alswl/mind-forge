//! T015 — Per-finding `boundary_mode` JSON contract (FR-003).
//!
//! `term lint --json` must surface a `boundary_mode` field on every finding
//! whose value is one of `"loose" | "standalone" | "cjk"`. The value tracks
//! which physical scanner mode produced the finding:
//!   * `loose`      — pinyin matches and ASCII `boundary: loose` corrections
//!   * `standalone` — ASCII corrections under `boundary: standalone`
//!   * `cjk`        — CJK originals (boundary field is ignored)

use assert_cmd::Command;
use std::fs;

mod common;

fn mf(repo: &common::TempDir) -> Command {
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.args(["--root", repo.path().to_str().unwrap()]);
    cmd
}

fn seed(repo: &common::TempDir, index_yaml: &str, doc: &str) {
    common::create_project(repo, "alpha");
    let project = repo.path().join("alpha");
    fs::create_dir_all(project.join("docs")).unwrap();
    common::write_index(repo, "alpha", index_yaml);
    fs::write(project.join("docs").join("note.md"), doc).unwrap();
}

fn lint_findings(repo: &common::TempDir) -> Vec<serde_json::Value> {
    let output = mf(repo).args(["term", "lint", "--project", "alpha", "--json"]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid JSON envelope");
    let findings = v["data"]["findings"].as_array().expect("data.findings array").clone();
    let expected_exit = if findings.is_empty() { 0 } else { 1 };
    assert_eq!(
        output.status.code(),
        Some(expected_exit),
        "lint exit code must reflect whether findings exist; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    findings
}

#[test]
fn finding_includes_boundary_mode_field() {
    let repo = common::setup_repo();
    seed(
        &repo,
        r#"schema_version: '1'
terms:
  - term: Mind Repo
    corrections:
      - original: mindrepo
        correct: Mind Repo
"#,
        "the mindrepo is here\n",
    );

    let findings = lint_findings(&repo);
    assert!(!findings.is_empty(), "expected at least one finding");
    for f in &findings {
        let mode = f["boundary_mode"].as_str().expect("boundary_mode field missing");
        assert!(
            matches!(mode, "loose" | "standalone" | "cjk"),
            "boundary_mode must be loose|standalone|cjk, got {mode:?}"
        );
    }
}

#[test]
fn ascii_word_default_boundary_is_standalone() {
    // After spec 046, `boundary` defaults to standalone. ASCII word corrections
    // therefore emit `boundary_mode: standalone`.
    let repo = common::setup_repo();
    seed(
        &repo,
        r#"schema_version: '1'
terms:
  - term: AIDC
    corrections:
      - original: aidc
        correct: AIDC
"#,
        "the aidc cluster\n",
    );

    let findings = lint_findings(&repo);
    assert!(!findings.is_empty(), "expected finding for aidc");
    assert_eq!(findings[0]["boundary_mode"], "standalone");
}

#[test]
fn explicit_loose_boundary_round_trips_to_mode() {
    let repo = common::setup_repo();
    seed(
        &repo,
        r#"schema_version: '1'
terms:
  - term: AIDC
    corrections:
      - original: aidc
        correct: AIDC
        boundary: loose
"#,
        "aidc-internal\n",
    );

    let findings = lint_findings(&repo);
    assert!(!findings.is_empty(), "loose boundary should match aidc-internal");
    assert_eq!(findings[0]["boundary_mode"], "loose");
}

#[test]
fn ascii_phrase_reports_standalone_boundary_mode() {
    // Standalone "foo dr" → "foodr" must use the ASCII standalone path (spec 054).
    let repo = common::setup_repo();
    seed(
        &repo,
        r#"schema_version: '1'
terms:
  - term: foodr
    corrections:
      - original: "foo dr"
        correct: foodr
        match: word
"#,
        "the foo dr site\n",
    );

    let findings = lint_findings(&repo);
    assert_eq!(findings.len(), 1, "expected exactly one finding for standalone 'the foo dr site', got {findings:#?}");
    let finding = &findings[0];
    assert_eq!(finding["match_kind"], "word", "expected match_kind=word, got {finding:#?}");
    assert_eq!(finding["correct"], "foodr", "expected correct=foodr, got {finding:#?}");
    assert_eq!(finding["boundary_mode"], "standalone", "unexpected boundary mode: {finding:#?}");
}

#[test]
fn cjk_original_emits_cjk_mode() {
    let repo = common::setup_repo();
    seed(
        &repo,
        r#"schema_version: '1'
terms:
  - term: 凯飞迪
    corrections:
      - original: 凯飞迪
        correct: 凯飞迪
"#,
        "凯飞迪 出现在这里\n",
    );

    let findings = lint_findings(&repo);
    assert!(!findings.is_empty(), "expected finding for CJK original");
    assert_eq!(findings[0]["boundary_mode"], "cjk");
}

// ═══════════════════════════════════════════════════════════════════════════════
// Known defect (spec 055 candidate) — mixed-script word boundary.
//
// Spec 054 fixed the all-ASCII phrase case by gating WordCheck on
// `original.is_ascii()`. The mixed-script case is the same bug class: any
// original containing a CJK char takes the whole-string CJK path, which only
// checks whether the immediate neighbour is a Han ideograph. So a mixed
// original ending in ASCII (e.g. "网关API") wrongly matches when glued to a
// longer ASCII word ("网关APInterface"): the right neighbour 'n' is non-CJK and
// counts as a boundary. Ignored until the per-edge boundary implementation lands.
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn mixed_script_word_not_glued_to_longer_ascii_word() {
    let repo = common::setup_repo();
    seed(
        &repo,
        r#"schema_version: '1'
terms:
  - term: 网关API
    corrections:
      - original: 网关API
        correct: 网关网关
        match: word
"#,
        "网关APInterface\n",
    );

    let findings = lint_findings(&repo);
    assert!(findings.is_empty(), "网关API must NOT match inside 网关APInterface, but found: {findings:#?}");
}
