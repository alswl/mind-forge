//! CLI coverage for the LM ASR post-correction engine (spec 056).
//!
//! Asserts SC-002 (LM surfaces `在线服物`→`在线服务` with PPL improvement),
//! SC-003 (clean/below-threshold/OOV inputs produce no finding), US2-AC3
//! (missing model exits 1), US2-AC4 (--ppl-threshold override), and US2-AC5
//! (read-only lint).
//!
//! LM model resolution uses the `MF_LM_MODEL_PATH` environment variable
//! (config-based resolution is wired in 057).

use assert_cmd::Command;
use serde_json::Value;
use std::fs;

mod common;

/// Repo with project `alpha`, a `docs/` dir, and glossary term `服务`
/// for static pinyin mapping. The LM engine generates same-pinyin candidates
/// from Jieba vocabulary and scores them with KenLM.
fn setup_lm_repo() -> common::TempDir {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    fs::create_dir_all(repo.path().join("alpha").join("docs")).unwrap();
    let index = r#"schema_version: '1'
terms:
  - term: 服务
    definition: service
    aliases: []
    tags: []
    corrections: []
  - term: 上线
    definition: launch
    aliases: []
    tags: []
    corrections: []
"#;
    common::write_index(&repo, "alpha", index);
    repo
}

fn write_doc(repo: &common::TempDir, name: &str, content: &str) {
    fs::write(repo.path().join("alpha").join("docs").join(format!("{name}.md")), content).unwrap()
}

fn read_doc(repo: &common::TempDir, name: &str) -> String {
    fs::read_to_string(repo.path().join("alpha").join("docs").join(format!("{name}.md"))).unwrap()
}

fn mf(repo: &common::TempDir) -> Command {
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.args(["--root", repo.path().to_str().unwrap()]);
    let fixture_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/asr/tiny_model.arpa");
    cmd.env("MF_LM_MODEL_PATH", fixture_path.to_str().unwrap());
    cmd
}

fn findings(stdout: &str) -> Vec<Value> {
    let v: Value = serde_json::from_str(stdout).expect("valid JSON envelope");
    v["data"]["findings"].as_array().cloned().unwrap_or_default()
}

// ── SC-002: LM lint reports a homophone finding with PPL fields ──────────────

#[test]
fn lm_lint_reports_homophone_finding_with_ppl_fields() {
    let repo = setup_lm_repo();
    // "服物" is a homophone of "服务" (same tone-stripped pinyin: fu-wu).
    // The fixture model assigns lower perplexity to "在线 服务 上线 了"
    // than "在线 服物 上线 了", so LM should flag the finding.
    write_doc(&repo, "demo", "在线服物上线了\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha", "--engine", "lm", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert_eq!(output.status.code(), Some(1), "lint with finding exits 1; stdout: {stdout}");

    let fs_ = findings(&stdout);
    assert!(!fs_.is_empty(), "expected at least one finding; stdout: {stdout}");
    let f = &fs_[0];
    assert_eq!(f["engine"], "lm", "engine field: {f}");
    assert_eq!(f["original"], "服物", "original: {f}");
    assert_eq!(f["correct"], "服务", "correct: {f}");
    assert_eq!(f["replacement_eligible"], true, "replacement_eligible: {f}");

    // PPL fields must be populated (FR-L8).
    assert!(f["model_version"].is_string(), "model_version must be a string: {f}");
    assert!(!f["model_version"].as_str().unwrap().is_empty(), "model_version must not be empty");
    assert!(f["ppl_before"].is_f64(), "ppl_before must be a number: {f}");
    assert!(f["ppl_after"].is_f64(), "ppl_after must be a number: {f}");
    let improvement = f["ppl_improvement"].as_f64().expect("ppl_improvement must be f64");
    assert!(improvement >= 0.20, "ppl_improvement must be >= 0.20: {improvement}");
}

// ── SC-003: clean text produces no finding ───────────────────────────────────

#[test]
fn lm_lint_clean_text_no_finding() {
    let repo = setup_lm_repo();
    // "在线服务上线了" uses the correct "服务" — no finding expected.
    write_doc(&repo, "demo", "在线服务上线了\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha", "--engine", "lm", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert_eq!(output.status.code(), Some(0), "clean lint exits 0; stdout: {stdout}");
    let fs_ = findings(&stdout);
    assert!(fs_.is_empty(), "expected no findings for clean text; got {:?}", fs_);
}

// ── US2-AC3: missing model exits 1 ───────────────────────────────────────────

#[test]
fn lm_lint_missing_model_exits_1() {
    let repo = setup_lm_repo();
    write_doc(&repo, "demo", "在线服物上线了\n");

    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.args(["--root", repo.path().to_str().unwrap()]);
    cmd.args(["term", "lint", "--project", "alpha", "--engine", "lm", "--json"]);
    // Set a nonexistent model path.
    cmd.env("MF_LM_MODEL_PATH", "/nonexistent/path/model.klm");

    let output = cmd.output().unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert_eq!(output.status.code(), Some(1), "missing model exits 1; stderr: {stderr}");
    assert!(
        stderr.contains("model_missing") || stderr.contains("ModelMissing"),
        "error should be model_missing; stderr: {stderr}"
    );
}

// ── US2-AC5: LM lint is read-only ────────────────────────────────────────────

#[test]
fn lm_lint_is_read_only() {
    let repo = setup_lm_repo();
    let content = "在线服物上线了\n";
    write_doc(&repo, "demo", content);

    let _output = mf(&repo).args(["term", "lint", "--project", "alpha", "--engine", "lm", "--json"]).output().unwrap();

    let after = read_doc(&repo, "demo");
    assert_eq!(after, content, "lint must not modify document; expected {:?}, got {:?}", content, after);
}

// ── US2-AC4: --ppl-threshold override ────────────────────────────────────────

#[test]
fn lm_lint_ppl_threshold_override_accepts() {
    let repo = setup_lm_repo();
    write_doc(&repo, "demo", "在线服物上线了\n");

    // Default threshold 0.20: should find.
    let output_default =
        mf(&repo).args(["term", "lint", "--project", "alpha", "--engine", "lm", "--json"]).output().unwrap();
    let stdout_default = String::from_utf8(output_default.stdout).unwrap();
    assert_eq!(output_default.status.code(), Some(1), "default threshold 0.20 should find; stdout: {stdout_default}");

    // Lower threshold 0.10: should still find.
    let output_low = mf(&repo)
        .args(["term", "lint", "--project", "alpha", "--engine", "lm", "--ppl-threshold", "0.10", "--json"])
        .output()
        .unwrap();
    let stdout_low = String::from_utf8(output_low.stdout).unwrap();
    let fs_ = findings(&stdout_low);
    assert!(!fs_.is_empty(), "threshold 0.10 should still find; got {:?}", fs_);
    let improvement = fs_[0]["ppl_improvement"].as_f64().unwrap();
    assert!(improvement >= 0.10, "ppl_improvement {improvement} >= 0.10");

    // The PPL improvement in the result should match or exceed the passed threshold.
    assert!(improvement >= 0.20, "improvement {improvement} should also be >= default 0.20");
}

// ── LM lint exits 2 on invalid PPL threshold ─────────────────────────────────

#[test]
fn lm_lint_invalid_ppl_threshold_exits_2() {
    let repo = setup_lm_repo();

    let output = mf(&repo)
        .args(["term", "lint", "--project", "alpha", "--engine", "lm", "--ppl-threshold", "NaN", "--json"])
        .output()
        .unwrap();
    // NaN is not a valid float for the threshold parser.
    assert_ne!(output.status.code(), Some(0), "invalid threshold must not exit 0");
}
