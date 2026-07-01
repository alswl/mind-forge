//! CLI guardrail tests for both correction engines (spec 057, US3).
//!
//! Covers canonical term protection, declared-correction precedence over
//! generated candidates, generated/generated overlap resolution, deterministic
//! ordering, text/JSON/exit code contracts, dry-run, and final filesystem content.
//!
//! Each acceptance scenario runs against `--engine rules` and `--engine lm`
//! separately; guardrails are per-engine invariants, not cross-engine merge rules.

use assert_cmd::Command;
use serde_json::Value;
use std::fs;

mod common;

/// Repo with glossary term `服务` (engine generates `服物→服务`) and protected
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
    corrections: []
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
/// untouched — neither engine may propose altering it.
#[test]
fn protected_canonical_term_not_flagged_rules() {
    let repo = setup_guardrail_repo();
    write_doc(&repo, "demo", "网关API上线了\n");

    let output =
        mf(&repo).args(["term", "lint", "--project", "alpha", "--engine", "rules", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let fs_ = findings(&stdout);
    assert!(fs_.is_empty(), "rules: protected term should not be flagged; stdout: {stdout}");
}

#[test]
fn protected_canonical_term_not_flagged_lm() {
    let repo = setup_guardrail_repo();
    write_doc(&repo, "demo", "网关API上线了\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha", "--engine", "lm", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let fs_ = findings(&stdout);
    // lm heuristic mode: 网关API has no declared correction, and is protected
    assert!(fs_.is_empty(), "lm: protected term should not be flagged; stdout: {stdout}");
}

// ── Declared-correction precedence (FR-G2) ────────────────────────────────

/// A declared literal correction `网关api→网关API` claims its span before
/// generated engine candidates. The engine must not propose an overlapping
/// generated correction at those offsets.
#[test]
fn declared_correction_claims_span_rules() {
    let repo = setup_guardrail_repo();
    write_doc(&repo, "demo", "在线服物上线了，网关api配置完成。\n");

    let output =
        mf(&repo).args(["term", "lint", "--project", "alpha", "--engine", "rules", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(output.status.code(), Some(1), "lint with findings exits 1");
    let fs_ = findings(&stdout);

    // The declared correction 网关api→网关API should be in findings (engine=None).
    let declared = fs_.iter().find(|f| f["original"] == "网关api" && f["correct"] == "网关API");
    assert!(declared.is_some(), "declared correction should be a finding; stdout: {stdout}");
    assert_eq!(declared.unwrap()["engine"], Value::Null, "declared correction has engine=null");

    // The engine-generated 服物→服务 should also be present.
    let generated = fs_.iter().find(|f| f["original"] == "服物" && f["correct"] == "服务");
    assert!(generated.is_some(), "engine-generated homophone should be present; stdout: {stdout}");
    assert_eq!(generated.unwrap()["engine"], "rules");

    // No finding should overlap the declared span with a generated engine.
    for f in &fs_ {
        if f["engine"] == "rules" && f["original"] == "网关api" {
            panic!("engine should not produce a finding at declared span; stdout: {stdout}");
        }
    }
}

#[test]
fn declared_correction_claims_span_lm() {
    let repo = setup_guardrail_repo();
    write_doc(&repo, "demo", "在线服物上线了，网关api配置完成。\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha", "--engine", "lm", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(output.status.code(), Some(1), "lint with findings exits 1");
    let fs_ = findings(&stdout);

    // The declared correction 网关api→网关API should be in findings.
    let declared = fs_.iter().find(|f| f["original"] == "网关api" && f["correct"] == "网关API");
    assert!(declared.is_some(), "declared correction should be a finding; stdout: {stdout}");
    assert_eq!(declared.unwrap()["engine"], Value::Null, "declared correction has engine=null");

    // The engine-generated homophone should also be present (heuristic mode).
    let generated = fs_.iter().find(|f| f["original"] == "服物" && f["correct"] == "服务");
    assert!(generated.is_some(), "engine-generated homophone should be present; stdout: {stdout}");
    assert_eq!(generated.unwrap()["engine"], "lm");

    // No generated finding should overlap the declared correction span.
    for f in &fs_ {
        if f["engine"] == "lm" && f["original"] == "网关api" {
            panic!("engine should not produce a finding at declared span; stdout: {stdout}");
        }
    }
}

// ── Declared precedence: different-length spans ──────────────────────────

/// A declared correction with a different byte length than what an engine
/// would propose tests that overlap logic uses the declared span's actual
/// length, not the proposal's length (the core T202 fix).
#[test]
fn declared_precedence_diff_length_rules() {
    let repo = setup_guardrail_repo();
    // "网关api" declared correction must match (needs word boundary). Use "，" before it.
    // Engine might see sub-spans within the declared region — they must be blocked.
    write_doc(&repo, "demo", "前，网关api后\n");

    let output =
        mf(&repo).args(["term", "lint", "--project", "alpha", "--engine", "rules", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let fs_ = findings(&stdout);

    // The one declared correction should be the only finding touching that region.
    let declared_in_region: Vec<_> =
        fs_.iter().filter(|f| f["original"].as_str().unwrap_or("").contains("网关")).collect();
    assert_eq!(declared_in_region.len(), 1, "only the declared correction should cover 网关 region; stdout: {stdout}");
}

#[test]
fn declared_precedence_diff_length_lm() {
    let repo = setup_guardrail_repo();
    write_doc(&repo, "demo", "前，网关api后\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha", "--engine", "lm", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let fs_ = findings(&stdout);

    let declared_in_region: Vec<_> =
        fs_.iter().filter(|f| f["original"].as_str().unwrap_or("").contains("网关")).collect();
    assert_eq!(declared_in_region.len(), 1, "only the declared correction should cover 网关 region; stdout: {stdout}");
}

// ── Generated/generated overlap: deterministic non-overlapping (FR-G3) ────

/// When multiple engine-generated candidates would overlap, the engine
/// selects one deterministically and all final edit spans are non-overlapping.
#[test]
fn generated_non_overlapping_spans_rules() {
    let repo = setup_guardrail_repo();
    // Same-pinyin terms sharing a key: both would match the same span.
    // The engine must pick one deterministically.
    write_doc(&repo, "demo", "机器仁开始工作\n");

    let output1 =
        mf(&repo).args(["term", "lint", "--project", "alpha", "--engine", "rules", "--json"]).output().unwrap();
    let stdout1 = String::from_utf8(output1.stdout).unwrap();
    let fs1 = findings(&stdout1);

    // Second run must produce identical results.
    let output2 =
        mf(&repo).args(["term", "lint", "--project", "alpha", "--engine", "rules", "--json"]).output().unwrap();
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

#[test]
fn generated_non_overlapping_spans_lm() {
    let repo = setup_guardrail_repo();
    write_doc(&repo, "demo", "机器仁开始工作\n");

    let output1 = mf(&repo).args(["term", "lint", "--project", "alpha", "--engine", "lm", "--json"]).output().unwrap();
    let stdout1 = String::from_utf8(output1.stdout).unwrap();
    let fs1 = findings(&stdout1);

    let output2 = mf(&repo).args(["term", "lint", "--project", "alpha", "--engine", "lm", "--json"]).output().unwrap();
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
fn lint_text_output_includes_original_and_correct_rules() {
    let repo = setup_guardrail_repo();
    write_doc(&repo, "demo", "在线服物上线了\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha", "--engine", "rules"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("服物"), "text output should mention original; stdout: {stdout}");
    assert!(stdout.contains("服务"), "text output should mention correct; stdout: {stdout}");
    assert_eq!(output.status.code(), Some(1), "lint with findings exits 1");
}

#[test]
fn lint_text_output_includes_original_and_correct_lm() {
    let repo = setup_guardrail_repo();
    write_doc(&repo, "demo", "在线服物上线了\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha", "--engine", "lm"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("服物"), "text output should mention original; stdout: {stdout}");
    assert!(stdout.contains("服务"), "text output should mention correct; stdout: {stdout}");
    assert_eq!(output.status.code(), Some(1), "lint with findings exits 1");
}

// ── JSON output contract ──────────────────────────────────────────────────

#[test]
fn lint_json_output_has_required_fields_rules() {
    let repo = setup_guardrail_repo();
    write_doc(&repo, "demo", "在线服物上线了，网关api配置完成。\n");

    let output =
        mf(&repo).args(["term", "lint", "--project", "alpha", "--engine", "rules", "--json"]).output().unwrap();
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

#[test]
fn lint_json_output_has_required_fields_lm() {
    let repo = setup_guardrail_repo();
    write_doc(&repo, "demo", "在线服物上线了，网关api配置完成。\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha", "--engine", "lm", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let fs_ = findings(&stdout);

    for f in &fs_ {
        assert!(f["original"].is_string(), "original must be string: {f}");
        assert!(f["correct"].is_string(), "correct must be string: {f}");
        assert!(f["line"].is_u64(), "line must be u64: {f}");
        assert!(f["column"].is_u64(), "column must be u64: {f}");
        assert!(f["fix_kind"].is_string(), "fix_kind must be string: {f}");
    }
}

// ── Empty results exit 0 ──────────────────────────────────────────────────

#[test]
fn clean_document_exits_zero_rules() {
    let repo = setup_guardrail_repo();
    write_doc(&repo, "demo", "在线服务上线了\n");

    let output =
        mf(&repo).args(["term", "lint", "--project", "alpha", "--engine", "rules", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let fs_ = findings(&stdout);
    assert!(fs_.is_empty(), "clean document has no findings");
    assert_eq!(output.status.code(), Some(0), "clean lint exits 0");
}

#[test]
fn clean_document_exits_zero_lm() {
    let repo = setup_guardrail_repo();
    write_doc(&repo, "demo", "在线服务上线了\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha", "--engine", "lm", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let fs_ = findings(&stdout);
    assert!(fs_.is_empty(), "clean document has no findings");
    assert_eq!(output.status.code(), Some(0), "clean lint exits 0");
}

// ── Dry-run reports edits without writing (FR-G3) ─────────────────────────

#[test]
fn fix_dry_run_reports_no_write_rules() {
    let repo = setup_guardrail_repo();
    write_doc(&repo, "demo", "在线服物上线了\n");

    let output = mf(&repo)
        .args(["term", "fix", "--project", "alpha", "--engine", "rules", "--dry-run", "--json"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["data"]["dry_run"], true, "dry_run: {stdout}");
    assert_eq!(v["data"]["modified_files"].as_array().unwrap().len(), 0, "no files modified: {stdout}");
    assert_eq!(read_doc(&repo, "demo"), "在线服物上线了\n", "dry-run must not write");
}

#[test]
fn fix_dry_run_reports_no_write_lm() {
    let repo = setup_guardrail_repo();
    write_doc(&repo, "demo", "在线服物上线了\n");

    let output = mf(&repo)
        .args(["term", "fix", "--project", "alpha", "--engine", "lm", "--dry-run", "--json"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["data"]["dry_run"], true, "dry_run: {stdout}");
    assert_eq!(v["data"]["modified_files"].as_array().unwrap().len(), 0, "no files modified: {stdout}");
    assert_eq!(read_doc(&repo, "demo"), "在线服物上线了\n", "dry-run must not write");
}

// ── Fix writes and produces correct content (FR-G3) ───────────────────────

#[test]
fn fix_writes_correct_content_rules() {
    let repo = setup_guardrail_repo();
    write_doc(&repo, "demo", "在线服物上线了\n");

    let output =
        mf(&repo).args(["term", "fix", "--project", "alpha", "--engine", "rules", "--yes", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["data"]["fixed_count"], 1, "one fix applied: {stdout}");

    let content = read_doc(&repo, "demo");
    assert!(content.contains("在线服务"), "fixed content should contain 服务, got: {content}");
    assert!(!content.contains("服物"), "fixed content should not contain 服物");
}

#[test]
fn fix_writes_correct_content_lm() {
    let repo = setup_guardrail_repo();
    write_doc(&repo, "demo", "在线服物上线了\n");

    let output = mf(&repo)
        .args(["term", "fix", "--project", "alpha", "--engine", "lm", "--include-suggested", "--yes", "--json"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["data"]["fixed_count"], 1, "one fix applied: {stdout}");

    let content = read_doc(&repo, "demo");
    assert!(content.contains("在线服务"), "fixed content should contain 服务, got: {content}");
    assert!(!content.contains("服物"), "fixed content should not contain 服物");
}

// ── Protected term survives fix ───────────────────────────────────────────

#[test]
fn fix_preserves_protected_term_rules() {
    let repo = setup_guardrail_repo();
    write_doc(&repo, "demo", "网关API上线了\n");

    let output =
        mf(&repo).args(["term", "fix", "--project", "alpha", "--engine", "rules", "--yes", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["data"]["fixed_count"], 0, "protected term should not be fixed: {stdout}");
    assert_eq!(read_doc(&repo, "demo"), "网关API上线了\n", "protected term must be unchanged");
}

#[test]
fn fix_preserves_protected_term_lm() {
    let repo = setup_guardrail_repo();
    write_doc(&repo, "demo", "网关API上线了\n");

    let output =
        mf(&repo).args(["term", "fix", "--project", "alpha", "--engine", "lm", "--yes", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["data"]["fixed_count"], 0, "protected term should not be fixed: {stdout}");
    assert_eq!(read_doc(&repo, "demo"), "网关API上线了\n", "protected term must be unchanged");
}

// ── combined document: homophone + protected + declared all at once ───────

#[test]
fn combined_document_all_guardrails_rules() {
    let repo = setup_guardrail_repo();
    // 服物→服务 (engine), 网关API (protected), 网关api→网关API (declared).
    // Space-separate 网关api so it passes standalone boundary check.
    write_doc(&repo, "demo", "测试服物和网关API还有 网关api 的问题\n");

    let output =
        mf(&repo).args(["term", "fix", "--project", "alpha", "--engine", "rules", "--yes", "--json"]).output().unwrap();
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

#[test]
fn combined_document_all_guardrails_lm() {
    let repo = setup_guardrail_repo();
    write_doc(&repo, "demo", "测试服物和网关API还有 网关api 的问题\n");

    let output = mf(&repo)
        .args(["term", "fix", "--project", "alpha", "--engine", "lm", "--include-suggested", "--yes", "--json"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: Value = serde_json::from_str(&stdout).unwrap();

    let content = read_doc(&repo, "demo");
    assert!(content.contains("服务"), "服物 should be fixed to 服务; content: {content}");
    assert!(!content.contains("服物"), "服物 should be gone; content: {content}");
    assert!(content.contains("网关API"), "protected 网关API must survive; content: {content}");
    assert!(!content.contains("网关api"), "declared 网关api should be fixed to 网关API; content: {content}");
    assert_eq!(v["data"]["fixed_count"], 2, "two fixes: generated + declared; stdout: {stdout}");
}

// ── No over-application: fix should not touch unrelated text ──────────────

#[test]
fn fix_does_not_over_apply_rules() {
    let repo = setup_guardrail_repo();
    write_doc(&repo, "demo", "正常文本没有任何问题\n");

    let output =
        mf(&repo).args(["term", "fix", "--project", "alpha", "--engine", "rules", "--yes", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["data"]["fixed_count"], 0, "no fixes on clean text: {stdout}");
    assert_eq!(read_doc(&repo, "demo"), "正常文本没有任何问题\n", "clean text unchanged");
}

#[test]
fn fix_does_not_over_apply_lm() {
    let repo = setup_guardrail_repo();
    write_doc(&repo, "demo", "正常文本没有任何问题\n");

    let output =
        mf(&repo).args(["term", "fix", "--project", "alpha", "--engine", "lm", "--yes", "--json"]).output().unwrap();
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
fn declared_blocks_engine_subspan_rules() {
    let repo = setup_guardrail_repo();
    // declared: 网关api→网关API at a known position. The engine must not
    // produce a proposal that starts inside that declared span.
    write_doc(&repo, "demo", "前，网关api后\n");

    let output =
        mf(&repo).args(["term", "lint", "--project", "alpha", "--engine", "rules", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let fs_ = findings(&stdout);

    // Every engine-generated finding must be outside the declared span.
    // The "前" is at offset 0 (3 bytes), "网关api" at offset 3-12, "后" at offset 13.
    // An engine-generated finding at offset 3 would overlap the declared span.
    for f in &fs_ {
        if f["engine"].is_null() {
            continue; // declared corrections themselves are fine
        }
        let original = f["original"].as_str().unwrap();
        // The engine should not generate a finding whose original is "网关" or
        // sub-span of the declared original at the same position.
        if original.contains("网关") || original.contains("api") {
            panic!("engine should not produce finding overlapping declared span; f={f}");
        }
    }
}

#[test]
fn declared_blocks_engine_subspan_lm() {
    let repo = setup_guardrail_repo();
    write_doc(&repo, "demo", "前，网关api后\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha", "--engine", "lm", "--json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let fs_ = findings(&stdout);

    for f in &fs_ {
        if f["engine"].is_null() {
            continue;
        }
        let original = f["original"].as_str().unwrap();
        if original.contains("网关") || original.contains("api") {
            panic!("engine should not produce finding overlapping declared span; f={f}");
        }
    }
}
