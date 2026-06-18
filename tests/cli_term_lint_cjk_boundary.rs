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

// ── Scenario 4: match: substring → 机器人在工厂 returns 1 finding ──

#[test]
fn cjk_substring_match_robot_matches() {
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
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!output.status.success(), "should exit 1 with a finding");
    assert!(stdout.contains("→ \"装置\""), "substring match must find '机器' inside '机器人': {stdout}");
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

    let output = mf(&repo).args(["term", "lint", "--project", "alpha", "--format", "json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"match_kind\""), "JSON finding must have match_kind: {stdout}");
    assert!(stdout.contains("\"word\""), "default match_kind should be 'word': {stdout}");
    assert!(stdout.contains("\"fix_kind\""), "JSON finding must have fix_kind: {stdout}");
    assert!(stdout.contains("\"required\""), "default fix_kind should be 'required': {stdout}");
}

#[test]
fn json_finding_has_match_kind_substring() {
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
    write_cjk_doc(&project, "cjk", "机器人在工厂\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha", "--format", "json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"match_kind\": \"substring\""), "JSON must have substring match_kind: {stdout}");
}
