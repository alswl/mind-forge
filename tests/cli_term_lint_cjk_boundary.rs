use assert_cmd::Command;
use std::fs;

mod common;

fn setup_cjk_repo() -> common::TempDir {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    fs::create_dir_all(project.join("docs")).unwrap();
    repo
}

fn write_cjk_doc(project: &std::path::Path, name: &str, content: &str) {
    fs::write(project.join("docs").join(format!("{name}.md")), content).unwrap();
}

fn write_cjk_index(repo: &common::TempDir, project_name: &str, yaml: &str) {
    common::write_index(repo, project_name, yaml);
}

fn mf(repo: &common::TempDir) -> Command {
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.args(["--root", repo.path().to_str().unwrap()]);
    cmd
}

// ── Scenario 1: match=word (default), 机器 + 机器人在工厂 → 0 findings ──

#[test]
fn cjk_word_boundary_machine_in_robot_not_matched() {
    let repo = setup_cjk_repo();
    let project = repo.path().join("alpha");
    let index_yaml = r#"schema_version: '1'
terms:
  - term: Device
    corrections:
      - original: 机器
        correct: 装置
"#;
    write_cjk_index(&repo, "alpha", index_yaml);
    write_cjk_doc(&project, "cjk", "机器人在工厂里很常见。\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("No term issues found."), "robots must not match '机器': {stdout}");
}

// ── Scenario 2: match=word (default), Python 机器 模型 → 1 finding ──

#[test]
fn cjk_word_boundary_machine_standalone_matched() {
    let repo = setup_cjk_repo();
    let project = repo.path().join("alpha");
    let index_yaml = r#"schema_version: '1'
terms:
  - term: Device
    corrections:
      - original: 机器
        correct: 装置
"#;
    write_cjk_index(&repo, "alpha", index_yaml);
    write_cjk_doc(&project, "cjk", "Python 机器 模型是热门。\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!output.status.success(), "should exit 1 with a finding");
    assert!(stdout.contains("→ \"装置\""), "standalone '机器' must match: {stdout}");
}

// ── Scenario 3: match=word (default), 使用机器。→ 1 finding (full-width punctuation neighbor) ──

#[test]
fn cjk_word_boundary_fullwidth_punctuation_neighbor() {
    let repo = setup_cjk_repo();
    let project = repo.path().join("alpha");
    let index_yaml = r#"schema_version: '1'
terms:
  - term: Device
    corrections:
      - original: 机器
        correct: 装置
"#;
    write_cjk_index(&repo, "alpha", index_yaml);
    write_cjk_doc(&project, "cjk", "使用机器。\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!output.status.success(), "should exit 1 with a finding");
    assert!(stdout.contains("→ \"装置\""), "right-neighbor '。' is non-CJK ideograph, must match: {stdout}");
}

// ── Scenario 4: standalone substring uses CJK token boundaries ──

#[test]
fn cjk_substring_standalone_suppresses_embedded_token() {
    let repo = setup_cjk_repo();
    let project = repo.path().join("alpha");
    let index_yaml = r#"schema_version: '1'
terms:
  - term: Device
    corrections:
      - original: 机器
        correct: 装置
        match: substring
"#;
    write_cjk_index(&repo, "alpha", index_yaml);
    write_cjk_doc(&project, "cjk", "机器人在工厂里很常见。\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha"]).output().unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "embedded token should be suppressed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// ── Scenario 5: invalid match value → YAML load error ──

#[test]
fn cjk_invalid_match_kind_rejected() {
    let repo = setup_cjk_repo();
    let index_yaml = r#"schema_version: '1'
terms:
  - term: Device
    corrections:
      - original: 机器
        correct: 装置
        match: bogus
"#;
    write_cjk_index(&repo, "alpha", index_yaml);

    let output = mf(&repo).args(["term", "lint", "--project", "alpha"]).output().unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(!output.status.success(), "bogus match must fail");
    assert!(
        stderr.contains("bogus") || stderr.contains("match") || stderr.contains("error"),
        "error must reference the problem: stderr={stderr}"
    );
}

// ── Scenario 6: JSON mode emits match_kind on every finding ──

#[test]
fn json_finding_has_match_kind_word() {
    let repo = setup_cjk_repo();
    let project = repo.path().join("alpha");
    let index_yaml = r#"schema_version: '1'
terms:
  - term: Device
    corrections:
      - original: 机器
        correct: 装置
"#;
    write_cjk_index(&repo, "alpha", index_yaml);
    write_cjk_doc(&project, "cjk", "Python 机器 模型\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha", "--output", "json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"match_kind\""), "JSON finding must have match_kind: {stdout}");
    assert!(stdout.contains("\"word\""), "default match_kind should be 'word': {stdout}");
    assert!(stdout.contains("\"fix_kind\""), "JSON finding must have fix_kind: {stdout}");
    assert!(stdout.contains("\"required\""), "default fix_kind should be 'required': {stdout}");
}

#[test]
fn json_finding_reports_loose_substring() {
    let repo = setup_cjk_repo();
    let project = repo.path().join("alpha");
    let index_yaml = r#"schema_version: '1'
terms:
  - term: Device
    corrections:
      - original: 机器
        correct: 装置
        match: substring
        boundary: loose
"#;
    write_cjk_index(&repo, "alpha", index_yaml);
    write_cjk_doc(&project, "cjk", "机器人在工厂\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha", "--output", "json"]).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("\"match_kind\": \"substring\"") || stdout.contains("\"match_kind\":\"substring\""),
        "JSON stdout: {stdout}"
    );
}

// ── US2 (Bug #8): CJK corrections fire in pure-CJK text (T014) ──

/// T014: `term lint` reports `时刻→实刻` in pure-Chinese text (Bug #8 fix).
#[test]
fn cjk_correction_fires_in_pure_cjk_text_lint() {
    let repo = setup_cjk_repo();
    let project = repo.path().join("alpha");
    let index_yaml = r#"schema_version: '1'
terms:
  - term: Moment
    corrections:
      - original: 时刻
        correct: 实刻
"#;
    write_cjk_index(&repo, "alpha", index_yaml);
    write_cjk_doc(&project, "cjk", "把握每一时刻，时刻！\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!output.status.success(), "should exit 1 with a finding");
    assert!(stdout.contains("时刻"), "must report 时刻 occurrence: {stdout}");
    assert!(stdout.contains("实刻"), "must suggest 实刻 correction: {stdout}");
}

/// T014: `term fix` replaces `时刻→实刻` with `--term Moment` (the explicit opt-in
/// for short-CJK advisory findings, spec 074 #30) plus --include-suggested -y
/// (SC-002 positive).
#[test]
fn cjk_correction_fix_replaces_in_pure_cjk_text() {
    let repo = setup_cjk_repo();
    let project = repo.path().join("alpha");
    let index_yaml = r#"schema_version: '1'
terms:
  - term: Moment
    corrections:
      - original: 时刻
        correct: 实刻
"#;
    write_cjk_index(&repo, "alpha", index_yaml);
    write_cjk_doc(&project, "cjk", "时刻\n");

    let output = mf(&repo)
        .args(["term", "fix", "--project", "alpha", "--term", "Moment", "--include-suggested", "-y"])
        .output()
        .unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(output.status.success(), "fix should succeed; stderr={stderr}");

    let fixed = fs::read_to_string(project.join("docs/cjk.md")).unwrap();
    assert!(!fixed.contains("时刻"), "时刻 should be replaced in fixed file: {fixed}");
    assert!(fixed.contains("实刻"), "实刻 should appear in fixed file: {fixed}");
}

/// Spec 074 #30: without the explicit opt-in, the short-CJK correction 时刻→实刻
/// is advisory and `term fix` must NOT apply it (prose stays intact).
#[test]
fn cjk_short_correction_not_auto_applied_without_opt_in() {
    let repo = setup_cjk_repo();
    let project = repo.path().join("alpha");
    let index_yaml = r#"schema_version: '1'
terms:
  - term: Moment
    corrections:
      - original: 时刻
        correct: 实刻
"#;
    write_cjk_index(&repo, "alpha", index_yaml);
    write_cjk_doc(&project, "cjk", "时刻\n");

    let output = mf(&repo).args(["term", "fix", "--project", "alpha", "--include-suggested", "-y"]).output().unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(output.status.success(), "fix should succeed; stderr={stderr}");

    let fixed = fs::read_to_string(project.join("docs/cjk.md")).unwrap();
    assert!(fixed.contains("时刻"), "时刻 (short-CJK advisory) must NOT be auto-applied: {fixed}");
}

// ── US2 (Bug #5): common words not clobbered (T015) ──

/// T015(a): sub-word originals rejected by jieba boundary (e.g. "小文" in "缩小文件").
#[test]
fn cjk_sub_word_rejected_by_jieba_boundary() {
    let repo = setup_cjk_repo();
    let project = repo.path().join("alpha");
    let index_yaml = r#"schema_version: '1'
terms:
  - term: Xiaowen
    corrections:
      - original: 小文
        correct: 晓文
"#;
    write_cjk_index(&repo, "alpha", index_yaml);
    write_cjk_doc(&project, "cjk", "缩小文件的方法。\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    // "缩小文件" → jieba tokens: 缩小 + 文件; "小文" crosses the boundary → rejected
    assert!(stdout.contains("No term issues found."), "小文 in 缩小文件 must NOT match: {stdout}");
}

/// T015(b): standalone common words registered as `suggested` are NOT applied
/// without `--include-suggested` (FR-005a fix-kind gating).
#[test]
fn cjk_suggested_not_applied_without_flag() {
    let repo = setup_cjk_repo();
    let project = repo.path().join("alpha");
    let index_yaml = r#"schema_version: '1'
terms:
  - term: Moon
    corrections:
      - original: 月亮
        correct: 月球
        fix: suggested
"#;
    write_cjk_index(&repo, "alpha", index_yaml);
    write_cjk_doc(&project, "cjk", "月亮\n");

    let output = mf(&repo).args(["term", "fix", "--project", "alpha", "-y"]).output().unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(output.status.success(), "fix should succeed; stderr={stderr}");

    let fixed = fs::read_to_string(project.join("docs/cjk.md")).unwrap();
    assert!(fixed.contains("月亮"), "suggested 月亮 must NOT be replaced without --include-suggested");
}

/// Loose substring remains available for intentional embedded CJK replacement.
#[test]
fn cjk_loose_substring_matches_in_sentence() {
    let repo = setup_cjk_repo();
    let project = repo.path().join("alpha");
    let index_yaml = r#"schema_version: '1'
terms:
  - term: Garbled
    corrections:
      - original: 卡机
        correct: 开机
        match: substring
        boundary: loose
"#;
    write_cjk_index(&repo, "alpha", index_yaml);
    write_cjk_doc(&project, "cjk", "系统卡机失败。\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha"]).output().unwrap();
    assert_eq!(output.status.code(), Some(1), "loose substring should emit a finding");
    assert!(String::from_utf8(output.stdout).unwrap().contains("开机"));
}

// ---------------------------------------------------------------------------
// Spec 075 US4: longest match wins in every mode.
// ---------------------------------------------------------------------------

fn setup_overlapping_corrections_repo() -> (common::TempDir, std::path::PathBuf) {
    let repo = setup_cjk_repo();
    let project = repo.path().join("alpha");
    let index_yaml = r#"schema_version: '1'
terms:
  - term: Device
    corrections:
      - original: 机器
        correct: 装置
  - term: Machinery
    corrections:
      - original: 机器人
        correct: 机械装置
"#;
    write_cjk_index(&repo, "alpha", index_yaml);
    (repo, project)
}

/// T065/FR-025: with a short correction ("机器") registered as a prefix of a
/// longer one ("机器人"), linting text containing the longer form must report
/// the longer correction and NOT the shorter one at that position.
#[test]
fn lint_reports_longest_match_not_the_shorter_prefix() {
    let (repo, project) = setup_overlapping_corrections_repo();
    write_cjk_doc(&project, "cjk", "机器人\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("→ \"机械装置\""), "the longer correction must be reported: {stdout}");
    assert!(!stdout.contains("→ \"装置\""), "the shorter correction must not fire at the same position: {stdout}");
}

/// T068/FR-029: the short correction still fires when standing alone outside
/// any longer match.
#[test]
fn lint_still_reports_short_correction_when_standing_alone() {
    let (repo, project) = setup_overlapping_corrections_repo();
    write_cjk_doc(&project, "cjk", "这个机器很旧了。\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("→ \"装置\""), "standalone shorter form must still be reported: {stdout}");
}

/// T070 (research U1 resolution attempt): the CJK word-boundary check (jieba
/// tokenization) already keeps a short correction from firing *inside* a
/// longer jieba-recognized word, which is why the two tests above pass
/// without any change. But `match: substring, boundary: loose` has no word
/// boundary at all (`WordCheck::SubstringLoose` is unconditional) — that is
/// where declaration-order claiming, not length, decides the winner. This
/// reproduces the actual U1 defect: the *shorter* correction is declared
/// first, so `scan_content`'s per-correction claimed-offset loop lets it
/// claim the position before the longer correction is ever tried.
#[test]
fn lint_loose_substring_reports_longest_match_regardless_of_declaration_order() {
    let repo = setup_cjk_repo();
    let project = repo.path().join("alpha");
    let index_yaml = r#"schema_version: '1'
terms:
  - term: Device
    corrections:
      - original: 机器
        correct: 装置
        match: substring
        boundary: loose
  - term: Machinery
    corrections:
      - original: 机器人
        correct: 机械装置
        match: substring
        boundary: loose
"#;
    write_cjk_index(&repo, "alpha", index_yaml);
    write_cjk_doc(&project, "cjk", "机器人\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("→ \"机械装置\""), "the longer correction must win at this position: {stdout}");
    assert!(
        !stdout.contains("→ \"装置\""),
        "the shorter correction (declared first) must not win by declaration order: {stdout}"
    );
}
