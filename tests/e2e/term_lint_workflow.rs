use std::fs;

use crate::helpers::*;

/// E2E: Cross-US workflow — US3 (CJK word-boundary) + US4 (term fix) + US5 (pinyin) + US6 (suggested)
///
/// Registers three corrections covering all 误改 scenarios:
///  - 机器 (default word, CJK)   → only match standalone, not embedded in 机器人
///  - 凯飞迪 (match: pinyin)     → finds 开飞地 via pinyin match, force suggested
///  - rag (fix: suggested)        → requires --include-suggested to apply
///
/// Then exercises: lint → fix -y (only required) → fix --include-suggested -y (all)
#[test]
fn e2e_term_lint_workflow_all_findings_then_fix() {
    // 1. Setup: create repo root with global terms + doc
    let ds = crate::datasets::Dataset::empty();
    let root = ds.root();
    let docs = root.join("docs");
    fs::create_dir_all(&docs).unwrap();

    // Write minds-terms.yaml with three corrections
    let terms_yaml = r#"schema_version: '1'
terms:
  - term: Device
    corrections:
      - original: 机器
        correct: 装置
  - term: Person:KaiFeidi
    corrections:
      - original: 凯飞迪
        correct: 凯飞迪
        match: pinyin
  - term: RAG
    corrections:
      - original: rag
        correct: RAG
        fix: suggested
"#;
    fs::write(root.join("minds-terms.yaml"), terms_yaml).unwrap();

    // Write document that tests all three scenarios
    let doc_content = "机器人在工厂很常见。\n会议由开飞地主持。\nwe use rag in production.\n";
    fs::write(docs.join("mixed.md"), doc_content).unwrap();

    // ── 2. Lint: verify all expected findings ──
    let (stdout, _stderr, code) = run_in(root, &["term", "lint", "docs/mixed.md"]);
    assert_eq!(code, 1, "lint should exit 1 with findings, got code={code} stdout={stdout} stderr={_stderr}");

    // US3: 机器 in 机器人 should NOT match (CJK word boundary — embedded in longer CJK word)
    assert!(!stdout.contains("机器人"), "CJK word boundary should not match embedded 机器: {stdout}");

    // US5: 开飞地 ≈ 凯飞迪 via pinyin
    assert!(stdout.contains("开飞地"), "pinyin finding for 开飞地 missing: {stdout}");

    // US6: rag should match
    assert!(stdout.contains("rag"), "suggested finding for rag missing: {stdout}");

    // Both pinyin and suggested findings should have ? suffix
    assert!(stdout.contains("?"), "suggested findings must have ? suffix: {stdout}");

    // ── 3. term fix -y: only required applied ──
    //    (机器 didn't match CJK boundary, so no required findings — file unchanged)
    let (_stdout, _stderr, code) = run_in(root, &["term", "fix", "docs/mixed.md", "-y"]);
    assert_eq!(code, 0, "fix should succeed");
    let content = fs::read_to_string(docs.join("mixed.md")).unwrap();
    assert!(content.contains("开飞地"), "suggested should not apply without --include-suggested: {content}");
    assert!(content.contains("rag"), "suggested should not apply without --include-suggested: {content}");

    // ── 4. term fix --include-suggested -y: applies suggested too ──
    let (_stdout, _stderr, code) = run_in(root, &["term", "fix", "docs/mixed.md", "--include-suggested", "-y"]);
    assert_eq!(code, 0, "fix --include-suggested should succeed");
    let content = fs::read_to_string(docs.join("mixed.md")).unwrap();
    assert!(content.contains("凯飞迪"), "pinyin fix should apply with --include-suggested: {content}");
    assert!(content.contains("RAG"), "suggested fix should apply with --include-suggested: {content}");
}

#[test]
fn e2e_precise_preview_matches_applied_changes() {
    let ds = crate::datasets::Dataset::empty();
    let root = ds.root();
    fs::write(
        root.join("minds-terms.yaml"),
        r#"schema_version: '1'
terms:
  - term: Alpha
    corrections:
      - original: alpha-old
        correct: Alpha
      - original: alpha-skip
        correct: Alpha
"#,
    )
    .unwrap();
    fs::write(root.join("synthetic.md"), "alpha-old alpha-skip\n").unwrap();

    let (stdout, stderr, code) =
        run_in(root, &["--output", "json", "term", "fix", "synthetic.md", "--term", "Alpha:alpha-old", "--dry-run"]);
    assert_eq!(code, 1, "preview should report findings: {stderr}");
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["data"]["selected_count"], 1);
    assert_eq!(value["data"]["ineligible_count"], 1);

    let (_, stderr, code) = run_in(root, &["term", "fix", "synthetic.md", "--term", "Alpha:alpha-old", "-y"]);
    assert_eq!(code, 0, "apply should succeed: {stderr}");
    assert_eq!(fs::read_to_string(root.join("synthetic.md")).unwrap(), "Alpha alpha-skip\n");
}
