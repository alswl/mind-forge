//! T009 + T012-T014 — Boundary fix tests and --include-suggested flag rename.
//!
//! T009: Integration test for the --all → --include-suggested rename:
//!   (a) --include-suggested controls suggested-correction inclusion
//!   (b) legacy --all produces identical behaviour AND a stderr WARN
//!   (c) JSON envelope data.warnings contains the deprecation line
//!
//! T012-T014: US1 safe-by-default boundary detection:
//!   T012: CJK fixture — standalone default only rewrites standalone lines
//!   T013: ASCII numeric — standalone default only rewrites standalone lines
//!   T014: explicit boundary: loose regression test

use assert_cmd::Command;
use std::fs;

mod common;

fn setup() -> (common::TempDir, std::path::PathBuf) {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    fs::create_dir_all(project.join("docs")).unwrap();
    (repo, project)
}

fn mf(repo: &common::TempDir) -> Command {
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.args(["--root", repo.path().to_str().unwrap()]);
    cmd
}

fn write_index(repo: &common::TempDir, yaml: &str) {
    common::write_index(repo, "alpha", yaml);
}

fn write_doc(repo: &common::TempDir, name: &str, content: &str) {
    let path = repo.path().join("alpha").join("docs").join(format!("{name}.md"));
    fs::write(path, content).unwrap();
}

// ═══════════════════════════════════════════════════════════════════════════════
// T009 — --include-suggested flag rename
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn include_suggested_flag_controls_suggested_inclusion() {
    let (repo, _project) = setup();
    let index = r#"schema_version: '1'
terms:
  - term: RAG
    corrections:
      - original: rag
        correct: RAG
        fix: suggested
"#;
    write_index(&repo, index);
    write_doc(&repo, "doc", "we use rag in production\n");

    // Without --include-suggested, suggested corrections are NOT applied.
    let output = mf(&repo).args(["term", "fix", "--project", "alpha", "docs/doc.md", "-y"]).output().unwrap();
    assert!(output.status.success());
    let doc = fs::read_to_string(repo.path().join("alpha/docs/doc.md")).unwrap();
    assert!(!doc.contains("RAG"), "without --include-suggested, suggested should NOT be applied: {doc}");

    // With --include-suggested, suggested corrections ARE applied.
    // Reset the doc first.
    write_doc(&repo, "doc", "we use rag in production\n");
    let output = mf(&repo)
        .args(["term", "fix", "--project", "alpha", "docs/doc.md", "--include-suggested", "-y"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let doc = fs::read_to_string(repo.path().join("alpha/docs/doc.md")).unwrap();
    assert!(doc.contains("RAG"), "--include-suggested must apply suggested corrections: {doc}");
}

#[test]
fn legacy_all_flag_produces_same_behaviour_and_warns() {
    let (repo, _project) = setup();
    let index = r#"schema_version: '1'
terms:
  - term: RAG
    corrections:
      - original: rag
        correct: RAG
        fix: suggested
"#;
    write_index(&repo, index);
    write_doc(&repo, "doc", "we use rag in production\n");

    let output = mf(&repo).args(["term", "fix", "--project", "alpha", "docs/doc.md", "--all", "-y"]).output().unwrap();
    assert!(output.status.success());

    // Legacy --all should produce stderr WARN.
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("WARN:") && stderr.contains("--all is deprecated; use --include-suggested instead"),
        "legacy --all must emit deprecation WARN on stderr: {stderr}"
    );

    // The suggested correction is still applied (same behaviour).
    let doc = fs::read_to_string(repo.path().join("alpha/docs/doc.md")).unwrap();
    assert!(doc.contains("RAG"), "legacy --all must still apply suggested corrections: {doc}");
}

#[test]
fn legacy_all_json_envelope_includes_warning() {
    let (repo, _project) = setup();
    let index = r#"schema_version: '1'
terms:
  - term: RAG
    corrections:
      - original: rag
        correct: RAG
        fix: suggested
"#;
    write_index(&repo, index);
    write_doc(&repo, "doc", "we use rag in production\n");

    let output = mf(&repo).args(["--project", "alpha", "--json", "term", "lint", "--all"]).output().unwrap();

    // Lint exits 1 when findings remain — that's expected. We only need JSON on stdout.
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid JSON envelope");
    let warnings = v["data"]["warnings"].as_array().expect("data.warnings array");
    assert!(
        warnings.iter().any(|w| w.as_str().unwrap_or("").contains("--all is deprecated")),
        "data.warnings must include the deprecation line: {warnings:?}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// T012 — CJK fixture: standalone default only rewrites standalone lines
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn cjk_standalone_default_only_rewrites_standalone_line() {
    let (repo, _project) = setup();
    // Correction: 小文 → <name>, no explicit boundary (defaults to Standalone)
    let index = r#"schema_version: '1'
terms:
  - term: "<name>"
    corrections:
      - original: 小文
        correct: "<name>"
"#;
    write_index(&repo, index);
    write_doc(&repo, "cjk", "小文件 备份策略\n小文 负责备份\n");

    let output = mf(&repo)
        .args(["term", "fix", "--project", "alpha", "docs/cjk.md", "--include-suggested", "-y"])
        .output()
        .unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let doc = fs::read_to_string(repo.path().join("alpha/docs/cjk.md")).unwrap();
    // 小文件 should NOT change (it's "小文" embedded in "小文件")
    assert!(doc.contains("小文件"), "小文件 should NOT be changed: {doc}");
    // 小文 (standalone) should change
    assert!(doc.contains("<name>"), "小文 (standalone) should be changed to <name>: {doc}");
    // Second line with 负责备份 should be preserved
    assert!(doc.contains("负责备份"), "second line context preserved: {doc}");
}

// ═══════════════════════════════════════════════════════════════════════════════
// T013 — ASCII numeric: standalone default only rewrites standalone lines
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn ascii_numeric_standalone_default_only_rewrites_standalone() {
    let (repo, _project) = setup();
    // Correction: 60 → 六十, no explicit boundary (defaults to Standalone)
    let index = r#"schema_version: '1'
terms:
  - term: 六十
    corrections:
      - original: "60"
        correct: 六十
"#;
    write_index(&repo, index);
    // "var60a" embeds "60" inside an ASCII identifier — must NOT change.
    // "他叫 60。" has "60" standalone — must change.
    write_doc(&repo, "num", "var60a is a symbol\n他叫 60。\n");

    let output = mf(&repo)
        .args(["term", "fix", "--project", "alpha", "docs/num.md", "--include-suggested", "-y"])
        .output()
        .unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let doc = fs::read_to_string(repo.path().join("alpha/docs/num.md")).unwrap();
    // var60a must NOT change (identifier-internal)
    assert!(doc.contains("var60a"), "var60a must NOT be changed: {doc}");
    // 他叫 60。 must change (standalone)
    assert!(doc.contains("他叫 六十"), "他叫 60。must change to 他叫 六十: {doc}");
}

// ═══════════════════════════════════════════════════════════════════════════════
// T014 — Regression: explicit boundary: loose keeps substring behaviour
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn explicit_loose_boundary_keeps_substring_behaviour() {
    let (repo, _project) = setup();
    let index = r#"schema_version: '1'
terms:
  - term: AIDC
    corrections:
      - original: aidc
        correct: AIDC
        boundary: loose
"#;
    write_index(&repo, index);
    write_doc(&repo, "ascii", "aidc-internal cluster\n");

    let output = mf(&repo)
        .args(["term", "fix", "--project", "alpha", "docs/ascii.md", "--include-suggested", "-y"])
        .output()
        .unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let doc = fs::read_to_string(repo.path().join("alpha/docs/ascii.md")).unwrap();
    // With boundary: loose, identifier-internal match works
    assert!(doc.contains("AIDC-internal"), "explicit boundary:loose should match inside identifiers: {doc}");
}
