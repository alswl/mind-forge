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

fn write_index(repo: &common::TempDir, yaml: &str) {
    common::write_index(repo, "alpha", yaml);
}

fn write_doc(project: &std::path::Path, name: &str, content: &str) {
    fs::write(project.join("docs").join(format!("{name}.md")), content).unwrap();
}

fn mf(repo: &common::TempDir) -> Command {
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.args(["--root", repo.path().to_str().unwrap()]);
    cmd
}

// ── Scenario 1: match: pinyin, 凯飞迪 vs 开飞地 → finding ──

#[test]
fn pinyin_kai_fei_di_match() {
    let repo = setup_cjk_repo();
    let project = repo.path().join("alpha");
    let index = r#"schema_version: '1'
terms:
  - term: Person:KaiFeidi
    corrections:
      - original: 凯飞迪
        correct: 凯飞迪
        match: pinyin
"#;
    write_index(&repo, index);
    write_doc(&project, "voice", "会议由开飞地主持。\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!output.status.success(), "should exit 1 with finding");
    assert!(stdout.contains("开飞地"), "must show original window text: {stdout}");
    assert!(stdout.contains("?"), "pinyin finding must have ? suffix: {stdout}");
}

// ── Scenario 2: 精研 vs 精盐 → pinyin match ──

#[test]
fn pinyin_jing_yan_match() {
    let repo = setup_cjk_repo();
    let project = repo.path().join("alpha");
    let index = r#"schema_version: '1'
terms:
  - term: 精研
    corrections:
      - original: 精研
        correct: 精研
        match: pinyin
"#;
    write_index(&repo, index);
    write_doc(&project, "voice", "使用精盐调味。\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!output.status.success(), "should exit 1 with finding");
    assert!(stdout.contains("精盐"), "must find 精盐 via pinyin: {stdout}");
}

// ── Scenario 3: explicit pinyin field overrides auto-conversion ──

#[test]
fn pinyin_explicit_field_override() {
    let repo = setup_cjk_repo();
    let project = repo.path().join("alpha");
    let index = r#"schema_version: '1'
terms:
  - term: 行
    corrections:
      - original: 行
        correct: 行
        match: pinyin
        pinyin: hang
"#;
    write_index(&repo, index);
    write_doc(&project, "multi", "他们在抗行。\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    // 抗行 pinyin = kang-hang, explicit pinyin = hang → mismatch → no finding
    assert!(output.status.success(), "explicit pinyin 'hang' should not match '抗行' (kang-hang): {stdout}");
}

// ── Scenario 4: match: pinyin + fix: required → forced suggested ──

#[test]
fn pinyin_always_suggested_even_if_configured_required() {
    let repo = setup_cjk_repo();
    let project = repo.path().join("alpha");
    let index = r#"schema_version: '1'
terms:
  - term: Person:KaiFeidi
    corrections:
      - original: 凯飞迪
        correct: 凯飞迪
        match: pinyin
        fix: required
"#;
    write_index(&repo, index);
    write_doc(&project, "voice", "会议由开飞地主持。\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha", "--format", "json"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"fix_kind\": \"suggested\""), "pinyin must be suggested regardless of config: {stdout}");
    assert!(!stdout.contains("\"fix_kind\": \"required\""), "pinyin finding must not be required: {stdout}");
}

// ── Scenario 5: ASCII original + match: pinyin → ignored (no CJK) ──

#[test]
fn pinyin_ascii_original_produces_no_finding() {
    let repo = setup_cjk_repo();
    let project = repo.path().join("alpha");
    let index = r#"schema_version: '1'
terms:
  - term: RAG
    corrections:
      - original: rag
        correct: RAG
        match: pinyin
"#;
    write_index(&repo, index);
    write_doc(&project, "ascii", "we use rag in production\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha"]).output().unwrap();
    assert!(output.status.success(), "ASCII+match:pinyin should produce no findings");
}

// ── Scenario 6: no CJK windows → 0 findings ──

#[test]
fn pinyin_no_cjk_in_doc_produces_no_finding() {
    let repo = setup_cjk_repo();
    let project = repo.path().join("alpha");
    let index = r#"schema_version: '1'
terms:
  - term: Person:KaiFeidi
    corrections:
      - original: 凯飞迪
        correct: 凯飞迪
        match: pinyin
"#;
    write_index(&repo, index);
    write_doc(&project, "ascii", "no chinese here at all\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha"]).output().unwrap();
    assert!(output.status.success(), "no CJK in doc: must be no finding");
}

// ── Scenario 7: FR-407 — exempt regions (code block / inline code) skip pinyin scan ──

#[test]
fn pinyin_skip_fenced_code_and_inline_code() {
    let repo = setup_cjk_repo();
    let project = repo.path().join("alpha");
    let index = r#"schema_version: '1'
terms:
  - term: Person:KaiFeidi
    corrections:
      - original: 凯飞迪
        correct: 凯飞迪
        match: pinyin
"#;
    write_index(&repo, index);
    // 开飞地 lives only inside a fenced code block and an inline code span.
    write_doc(&project, "code", "```\n会议由开飞地主持。\n```\n\n这是 `开飞地` 的引用。\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha"]).output().unwrap();
    assert!(output.status.success(), "exempt regions must not surface pinyin findings: {output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.contains("开飞地"), "pinyin must not match inside exempt regions: {stdout}");
}

// ── Scenario 8: fix with --include-suggested applies pinyin finding ──

#[test]
fn pinyin_fix_with_include_suggested_applies() {
    let repo = setup_cjk_repo();
    let project = repo.path().join("alpha");
    let index = r#"schema_version: '1'
terms:
  - term: Person:KaiFeidi
    corrections:
      - original: 凯飞迪
        correct: 凯飞迪
        match: pinyin
"#;
    write_index(&repo, index);
    write_doc(&project, "voice", "会议由开飞地主持。\n");

    let output = mf(&repo)
        .args(["term", "fix", "--project", "alpha", "docs/voice.md", "--include-suggested", "-y"])
        .output()
        .unwrap();
    assert!(output.status.success(), "--include-suggested fix should succeed");

    let doc = fs::read_to_string(project.join("docs/voice.md")).unwrap();
    assert!(doc.contains("凯飞迪"), "file must be corrected to 凯飞迪: {doc}");
}
