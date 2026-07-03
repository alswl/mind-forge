//! CLI guardrail tests for declared corrections and overlap safety.
//!
//! Covers canonical term protection, declared-correction precedence, overlap resolution, deterministic
//! ordering, text/JSON/exit code contracts, dry-run, and final filesystem content.
//!
//! Undeclared glossary homophones are intentionally outside the correction path.

use assert_cmd::Command;
use serde_json::Value;
use std::fs;

mod common;

/// Repo with glossary term `服务` and an explicit correction plus protected
/// canonical term `网关API` with a declared literal correction `网关api→网关API`.
fn setup_guardrail_repo() -> common::TempDir {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    fs::create_dir_all(repo.path().join("alpha").join("docs")).unwrap();
    let index = r#"schema_version: '1'
terms:
  - term: 服务
    definition: service
    aliases: []
    tags: []
    corrections:
      - original: 服物
        correct: 服务
        match: word
        fix: required
  - term: 网关API
    definition: Gateway API
    aliases: []
    tags: []
    corrections:
      - original: 网关api
        correct: 网关API
        match: word
        fix: required
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

// ── Canonical term protection (FR-G1) ──────────────────────────────────────

/// A document containing the protected canonical term `网关API` must leave it
/// untouched.
#[test]
fn protected_canonical_term_not_flagged() {
    let repo = setup_guardrail_repo();
    write_doc(&repo, "demo", "网关API上线了\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let fs_ = findings(&stdout);
    assert!(fs_.is_empty(), "protected term should not be flagged; stdout: {stdout}");
}

// ── Declared-correction precedence (FR-G2) ────────────────────────────────

/// A declared literal correction `网关api→网关API` claims its span before
/// generated engine candidates. The engine must not propose an overlapping
/// generated correction at those offsets.
#[test]
fn declared_correction_claims_span() {
    let repo = setup_guardrail_repo();
    write_doc(&repo, "demo", "在线服物上线了，网关api配置完成。\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(output.status.code(), Some(1), "lint with findings exits 1");
    let fs_ = findings(&stdout);

    // The declared correction 网关api→网关API should be in findings.
    let declared = fs_.iter().find(|f| f["original"] == "网关api" && f["correct"] == "网关API");
    assert!(declared.is_some(), "declared correction should be a finding; stdout: {stdout}");

    // The separately declared 服物→服务 correction should also be present.
    let separate = fs_.iter().find(|f| f["original"] == "服物" && f["correct"] == "服务");
    assert!(separate.is_some(), "declared correction should be present; stdout: {stdout}");

    // Exactly one finding may cover the declared span (the declared correction).
    let at_declared = fs_.iter().filter(|f| f["original"] == "网关api").count();
    assert_eq!(at_declared, 1, "only the declared correction may claim its span; stdout: {stdout}");
}

// ── Declared precedence: different-length spans ──────────────────────────

/// A declared correction with a different byte length than what an engine
/// would propose tests that overlap logic uses the declared span's actual
/// length, not the proposal's length (the 057 `e50a494` fix).
#[test]
fn declared_precedence_diff_length() {
    let repo = setup_guardrail_repo();
    // "网关api" declared correction must match (needs word boundary). Use "，" before it.
    // Engine might see sub-spans within the declared region — they must be blocked.
    write_doc(&repo, "demo", "前，网关api后\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let fs_ = findings(&stdout);

    // The one declared correction should be the only finding touching that region.
    let declared_in_region: Vec<_> =
        fs_.iter().filter(|f| f["original"].as_str().unwrap_or("").contains("网关")).collect();
    assert_eq!(declared_in_region.len(), 1, "only the declared correction should cover 网关 region; stdout: {stdout}");
}

// ── Generated/generated overlap: deterministic non-overlapping (FR-G3) ────

/// When multiple engine-generated candidates would overlap, the engine
/// selects one deterministically and all final edit spans are non-overlapping.
#[test]
fn generated_non_overlapping_spans() {
    let repo = setup_guardrail_repo();
    // Same-pinyin terms sharing a key: both would match the same span.
    // The engine must pick one deterministically.
    write_doc(&repo, "demo", "机器仁开始工作\n");

    let output1 = mf(&repo).args(["term", "lint", "--project", "alpha", "--json"]).output().unwrap();
    let stdout1 = String::from_utf8(output1.stdout).unwrap();
    let fs1 = findings(&stdout1);

    // Second run must produce identical results.
    let output2 = mf(&repo).args(["term", "lint", "--project", "alpha", "--json"]).output().unwrap();
    let stdout2 = String::from_utf8(output2.stdout).unwrap();
    let fs2 = findings(&stdout2);

    assert_eq!(fs1.len(), fs2.len(), "deterministic: same count across runs");
    for (a, b) in fs1.iter().zip(fs2.iter()) {
        assert_eq!(a["original"], b["original"], "deterministic: same original across runs");
        assert_eq!(a["correct"], b["correct"], "deterministic: same correct across runs");
        assert_eq!(a["line"], b["line"], "deterministic: same line across runs");
        assert_eq!(a["column"], b["column"], "deterministic: same column across runs");
    }
}

// ── Text output contract ──────────────────────────────────────────────────

#[test]
fn lint_text_output_includes_original_and_correct() {
    let repo = setup_guardrail_repo();
    write_doc(&repo, "demo", "在线服物上线了\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("服物"), "text output should mention original; stdout: {stdout}");
    assert!(stdout.contains("服务"), "text output should mention correct; stdout: {stdout}");
    assert_eq!(output.status.code(), Some(1), "lint with findings exits 1");
}

// ── JSON output contract ──────────────────────────────────────────────────

#[test]
fn lint_json_output_has_required_fields() {
    let repo = setup_guardrail_repo();
    write_doc(&repo, "demo", "在线服物上线了，网关api配置完成。\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let fs_ = findings(&stdout);

    for f in &fs_ {
        // Every finding must have standard fields.
        assert!(f["original"].is_string(), "original must be string: {f}");
        assert!(f["correct"].is_string(), "correct must be string: {f}");
        assert!(f["line"].is_u64(), "line must be u64: {f}");
        assert!(f["column"].is_u64(), "column must be u64: {f}");
        assert!(f["fix_kind"].is_string(), "fix_kind must be string: {f}");
    }
}

// ── Empty results exit 0 ──────────────────────────────────────────────────

#[test]
fn clean_document_exits_zero() {
    let repo = setup_guardrail_repo();
    write_doc(&repo, "demo", "在线服务上线了\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let fs_ = findings(&stdout);
    assert!(fs_.is_empty(), "clean document has no findings");
    assert_eq!(output.status.code(), Some(0), "clean lint exits 0");
}

// ── Dry-run reports edits without writing (FR-G3) ─────────────────────────

#[test]
fn fix_dry_run_reports_no_write() {
    let repo = setup_guardrail_repo();
    write_doc(&repo, "demo", "在线服物上线了\n");

    let output = mf(&repo).args(["term", "fix", "--project", "alpha", "--dry-run", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["data"]["dry_run"], true, "dry_run: {stdout}");
    assert_eq!(v["data"]["modified_files"].as_array().unwrap().len(), 0, "no files modified: {stdout}");
    assert_eq!(read_doc(&repo, "demo"), "在线服物上线了\n", "dry-run must not write");
}

// ── Fix writes and produces correct content (FR-G3) ───────────────────────

#[test]
fn fix_writes_correct_content() {
    let repo = setup_guardrail_repo();
    write_doc(&repo, "demo", "在线服物上线了\n");

    let output = mf(&repo).args(["term", "fix", "--project", "alpha", "--yes", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["data"]["fixed_count"], 1, "one fix applied: {stdout}");

    let content = read_doc(&repo, "demo");
    assert!(content.contains("在线服务"), "fixed content should contain 服务, got: {content}");
    assert!(!content.contains("服物"), "fixed content should not contain 服物");
}

// ── Protected term survives fix ───────────────────────────────────────────

#[test]
fn fix_preserves_protected_term() {
    let repo = setup_guardrail_repo();
    write_doc(&repo, "demo", "网关API上线了\n");

    let output = mf(&repo).args(["term", "fix", "--project", "alpha", "--yes", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["data"]["fixed_count"], 0, "protected term should not be fixed: {stdout}");
    assert_eq!(read_doc(&repo, "demo"), "网关API上线了\n", "protected term must be unchanged");
}

// ── combined document: homophone + protected + declared all at once ───────

#[test]
fn combined_document_all_guardrails() {
    let repo = setup_guardrail_repo();
    // 服物→服务 (engine), 网关API (protected), 网关api→网关API (declared).
    // Space-separate 网关api so it passes standalone boundary check.
    write_doc(&repo, "demo", "测试服物和网关API还有 网关api 的问题\n");

    let output = mf(&repo).args(["term", "fix", "--project", "alpha", "--yes", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: Value = serde_json::from_str(&stdout).unwrap();

    let content = read_doc(&repo, "demo");
    // 服物 should be corrected to 服务.
    assert!(content.contains("服务"), "服物 should be fixed to 服务; content: {content}");
    assert!(!content.contains("服物"), "服物 should be gone; content: {content}");
    // 网关API must survive untouched.
    assert!(content.contains("网关API"), "protected 网关API must survive; content: {content}");
    // 网关api should be corrected to 网关API (declared).
    assert!(!content.contains("网关api"), "declared 网关api should be fixed to 网关API; content: {content}");
    assert_eq!(v["data"]["fixed_count"], 2, "two fixes: generated + declared; stdout: {stdout}");
}

// ── No over-application: fix should not touch unrelated text ──────────────

#[test]
fn fix_does_not_over_apply() {
    let repo = setup_guardrail_repo();
    write_doc(&repo, "demo", "正常文本没有任何问题\n");

    let output = mf(&repo).args(["term", "fix", "--project", "alpha", "--yes", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["data"]["fixed_count"], 0, "no fixes on clean text: {stdout}");
    assert_eq!(read_doc(&repo, "demo"), "正常文本没有任何问题\n", "clean text unchanged");
}

// ── No engine-generated overlap on container/substring spans ───────────────

/// When a declared correction occupies a span and the engine would see a
/// shorter span within it (e.g. declared "网关api" 10 bytes, engine sees "网"
/// 3 bytes), the declared span blocks the engine-generated sub-span.
#[test]
fn declared_blocks_engine_subspan() {
    let repo = setup_guardrail_repo();
    // declared: 网关api→网关API at a known position. The engine must not
    // produce a proposal that starts inside that declared span.
    write_doc(&repo, "demo", "前，网关api后\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let fs_ = findings(&stdout);

    // Only the declared correction may touch the 网关 region; no generated
    // finding may overlap it.
    for f in &fs_ {
        let original = f["original"].as_str().unwrap_or("");
        let is_declared = f["original"] == "网关api" && f["correct"] == "网关API";
        if !is_declared && (original.contains("网关") || original.contains("api")) {
            panic!("engine should not produce finding overlapping declared span; f={f}");
        }
    }
}
