use assert_cmd::Command;
use std::fs;

mod common;

fn setup_with_term() -> (common::TempDir, std::path::PathBuf) {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    fs::create_dir_all(project.join("docs")).unwrap();
    // Seed a term with a correction
    let index_yaml = r#"schema_version: '1'
terms:
  - term: Mind Repo
    definition: 项目仓库根
    aliases:
      - mr
    tags:
      - infra
    corrections:
      - original: mindrepo
        correct: Mind Repo
"#;
    common::write_index(&repo, "alpha", index_yaml);
    (repo, project)
}

fn write_doc(project: &std::path::Path, name: &str, content: &str) {
    fs::write(project.join("docs").join(format!("{name}.md")), content).unwrap();
}

fn mf(repo: &common::TempDir) -> Command {
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.args(["--root", repo.path().to_str().unwrap()]);
    cmd
}

// ═══════════════════════════════════════════════════════════════════════════
// US3 — lint 只读
// ═══════════════════════════════════════════════════════════════════════════

// ---------------------------------------------------------------------------
// 1. single file single hit
// ---------------------------------------------------------------------------

#[test]
fn lint_basic_finding_shape() {
    let (repo, project) = setup_with_term();
    write_doc(&project, "intro", "hello mindrepo world\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha"]).output().unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("mindrepo"), "should find mindrepo: {stdout}");
    assert!(stdout.contains("Mind Repo"), "should suggest Mind Repo: {stdout}");
    assert!(!output.status.success(), "should exit 1 with findings");
}

// ---------------------------------------------------------------------------
// 2. exit 1 when findings present
// ---------------------------------------------------------------------------

#[test]
fn lint_exit_1_when_findings_present() {
    let (repo, project) = setup_with_term();
    write_doc(&project, "intro", "mindrepo is here\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha"]).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
}

// ---------------------------------------------------------------------------
// 3. exit 0 when clean
// ---------------------------------------------------------------------------

#[test]
fn lint_exit_0_when_clean() {
    let (repo, project) = setup_with_term();
    write_doc(&project, "intro", "no typos here\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("No term issues found."));
}

// ---------------------------------------------------------------------------
// 4. skips fenced code blocks
// ---------------------------------------------------------------------------

#[test]
fn lint_skips_fenced_code_block() {
    let (repo, project) = setup_with_term();
    write_doc(&project, "code", "outside mindrepo here\n```\ninside mindrepo block\n```\nafter mindrepo\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    // Only 2 findings (outside the fenced block); each finding contains "Mind Repo"
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout.matches("→ \"Mind Repo\"").count(), 2, "only 2 hits: {stdout}");
}

// ---------------------------------------------------------------------------
// 5. skips inline code, HTML comment, URL
// ---------------------------------------------------------------------------

#[test]
fn lint_skips_inline_code_html_comment_url() {
    let (repo, project) = setup_with_term();
    write_doc(
        &project,
        "exempt",
        "outside mindrepo\n`inside mindrepo code`\n<!-- mindrepo comment -->\nhttps://mindrepo.example.com\noutside again\n",
    );

    let output = mf(&repo).args(["term", "lint", "--project", "alpha"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    // Only 1 outside hit (mindrepo in HTML comment should be exempted)
    assert_eq!(stdout.matches("→ \"Mind Repo\"").count(), 1, "only 1 real hit: {stdout}");
}

// ---------------------------------------------------------------------------
// 6. front-matter skip
// ---------------------------------------------------------------------------

#[test]
fn lint_skips_file_via_front_matter() {
    let (repo, project) = setup_with_term();
    write_doc(&project, "skipped", "---\nmf_term_lint: skip\n---\nmindrepo inside skipped file\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha"]).output().unwrap();
    assert!(output.status.success(), "no findings");
    let stdout = String::from_utf8(output.stdout).unwrap();
    // If scanned_files 0 and no terms... actually we have terms so it should say "No term issues found."
    assert!(stdout.contains("No term issues found.") || stdout.contains("0 findings"));
}

// ---------------------------------------------------------------------------
// 7. HTML marker off/on
// ---------------------------------------------------------------------------

#[test]
fn lint_skips_block_via_html_marker() {
    let (repo, project) = setup_with_term();
    write_doc(
        &project,
        "markers",
        "before mindrepo\n<!-- mf-term-lint:off -->\ninside mindrepo off\n<!-- mf-term-lint:on -->\nafter mindrepo\n",
    );

    let output = mf(&repo).args(["term", "lint", "--project", "alpha"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.matches("→ \"Mind Repo\"").count(), 2, "before + after = 2: {stdout}");
}

// ---------------------------------------------------------------------------
// 8. multiple originals on same line
// ---------------------------------------------------------------------------

#[test]
fn lint_multiple_originals_on_same_line() {
    let (repo, project) = setup_with_term();
    write_doc(&project, "multi", "mindrepo and mindrepo on same line\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.matches("→ \"Mind Repo\"").count(), 2);
}

// ---------------------------------------------------------------------------
// 9. conflict: first term claims the original
// ---------------------------------------------------------------------------

#[test]
fn lint_conflict_first_term_claims() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    fs::create_dir_all(project.join("docs")).unwrap();
    // Two terms both correcting "conflict" → "Correct-A" and "conflict" → "Correct-B"
    // creates an ambiguous finding — neither should auto-replace.
    let index_yaml = r#"schema_version: '1'
terms:
  - term: Correct-A
    corrections:
      - original: conflict
        correct: Correct-A
  - term: Correct-B
    corrections:
      - original: conflict
        correct: Correct-B
"#;
    common::write_index(&repo, "alpha", index_yaml);
    write_doc(&project, "doc", "this is conflict word\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    // Ambiguous: both terms should be reported, not silently won by first
    assert!(stdout.contains("ambiguous"), "should be ambiguous: {stdout}");
    assert!(stdout.contains("Correct-A"), "should mention Correct-A: {stdout}");
    assert!(stdout.contains("Correct-B"), "should mention Correct-B: {stdout}");
}

// ---------------------------------------------------------------------------
// 10. JSON envelope
// ---------------------------------------------------------------------------

#[test]
fn lint_json_shape() {
    let (repo, project) = setup_with_term();
    write_doc(&project, "intro", "mindrepo here\n");

    let output = mf(&repo).args(["--format", "json", "term", "lint", "--project", "alpha"]).output().unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["status"], "ok");
    let findings = parsed["data"]["findings"].as_array().unwrap();
    assert_eq!(findings.len(), 1);
    let f = &findings[0];
    assert!(f.get("match_kind").is_some(), "finding must have match_kind");
    assert!(f.get("fix_kind").is_some(), "finding must have fix_kind");
    assert!(parsed["data"].get("scanned_files").is_some());
    assert!(parsed["data"].get("fixed_count").is_some());
    assert!(parsed["data"].get("modified_files").is_some());
    assert!(parsed["data"].get("failures").is_some());
    assert!(parsed["data"].get("would_apply_count").is_some(), "report must have would_apply_count");
}

// ---------------------------------------------------------------------------
// 11. no terms registered
// ---------------------------------------------------------------------------

#[test]
fn lint_no_terms_registered() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    fs::create_dir_all(project.join("docs")).unwrap();
    common::write_index(&repo, "alpha", "schema_version: '1'\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("No terms registered."));
}

// ---------------------------------------------------------------------------
// 12. corrections=[] doesn't cause errors
// ---------------------------------------------------------------------------

#[test]
fn lint_term_with_empty_corrections() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    fs::create_dir_all(project.join("docs")).unwrap();
    let index_yaml = r#"schema_version: '1'
terms:
  - term: Mind Repo
    corrections: []
"#;
    common::write_index(&repo, "alpha", index_yaml);
    write_doc(&project, "intro", "mindrepo\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("No term issues found."));
}

// ---------------------------------------------------------------------------
// 13. --project alpha --root /repo
// ---------------------------------------------------------------------------

#[test]
fn lint_with_project_root_flags() {
    // Already using --root in mf(); just verify --project works
    let (repo, project) = setup_with_term();
    write_doc(&project, "intro", "mindrepo\n");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "lint", "--project", "alpha"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
}

// ═══════════════════════════════════════════════════════════════════════════
// US4 — lint --fix
// ═══════════════════════════════════════════════════════════════════════════

// ---------------------------------------------------------------------------
// 1. single file, single hit, atomic write
// ---------------------------------------------------------------------------

#[test]
fn lint_fix_writes_back_atomically() {
    let (repo, project) = setup_with_term();
    write_doc(&project, "intro", "hello mindrepo world\n");

    let output = mf(&repo).args(["term", "lint", "--fix", "-y", "--project", "alpha"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("fixed"), "stdout: {stdout}");

    // Verify file content
    let content = fs::read_to_string(project.join("docs/intro.md")).unwrap();
    assert!(content.contains("Mind Repo"), "should be fixed: {content}");
    assert!(!content.contains("mindrepo"), "should replace original: {content}");
}

// ---------------------------------------------------------------------------
// 2. multiple findings, single atomic write
// ---------------------------------------------------------------------------

#[test]
fn lint_fix_multiple_findings_single_atomic() {
    let (repo, project) = setup_with_term();
    write_doc(&project, "intro", "mindrepo at start\nmindrepo at line 2\n");

    mf(&repo).args(["term", "lint", "--fix", "-y", "--project", "alpha"]).assert().code(0);

    let content = fs::read_to_string(project.join("docs/intro.md")).unwrap();
    assert_eq!(content.matches("Mind Repo").count(), 2);
    assert!(!content.contains("mindrepo"));
}

// ---------------------------------------------------------------------------
// 3. same original multiple times on same line
// ---------------------------------------------------------------------------

#[test]
fn lint_fix_replaces_all_occurrences_in_line() {
    let (repo, project) = setup_with_term();
    write_doc(&project, "intro", "mindrepo and mindrepo on same line\n");

    mf(&repo).args(["term", "lint", "--fix", "-y", "--project", "alpha"]).assert().code(0);

    let content = fs::read_to_string(project.join("docs/intro.md")).unwrap();
    assert_eq!(content.matches("Mind Repo").count(), 2);
}

// ---------------------------------------------------------------------------
// 4. original == correct — no write
// ---------------------------------------------------------------------------

#[test]
fn lint_fix_skips_when_original_equals_correct() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    fs::create_dir_all(project.join("docs")).unwrap();
    let index_yaml = r#"schema_version: '1'
terms:
  - term: Foo
    corrections:
      - original: foo
        correct: foo
"#;
    common::write_index(&repo, "alpha", index_yaml);
    write_doc(&project, "doc", "foo bar\n");

    let output = mf(&repo).args(["term", "lint", "--fix", "-y", "--project", "alpha"]).output().unwrap();
    // original == correct filtering: finding reported but no fix applied
    // Exit 0 because no actual failures
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("0 fixed"), "no fixes applied: {stdout}");
}

// ---------------------------------------------------------------------------
// 5. dry-run doesn't modify files
// ---------------------------------------------------------------------------

#[test]
fn lint_fix_dry_run_no_writes() {
    let (repo, project) = setup_with_term();
    write_doc(&project, "intro", "mindrepo here\n");

    let output = mf(&repo).args(["term", "lint", "--fix", "--dry-run", "--project", "alpha"]).output().unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("dry-run"), "stdout: {stdout}");

    // File should NOT be modified
    let content = fs::read_to_string(project.join("docs/intro.md")).unwrap();
    assert!(content.contains("mindrepo"), "should NOT fix: {content}");
}

// ---------------------------------------------------------------------------
// 6. respect exemption, skip fix
// ---------------------------------------------------------------------------

#[test]
fn lint_fix_respects_exemption() {
    let (repo, project) = setup_with_term();
    // File with front-matter skip
    write_doc(&project, "skipped", "---\nmf_term_lint: skip\n---\nmindrepo inside skipped\n");
    // Normal file
    write_doc(&project, "normal", "mindrepo here\n");

    mf(&repo).args(["term", "lint", "--fix", "-y", "--project", "alpha"]).assert().code(0);

    // Skipped file should not be modified
    let skipped = fs::read_to_string(project.join("docs/skipped.md")).unwrap();
    assert!(skipped.contains("mindrepo"), "skipped file unchanged: {skipped}");

    // Normal file should be fixed
    let normal = fs::read_to_string(project.join("docs/normal.md")).unwrap();
    assert!(normal.contains("Mind Repo"), "normal file fixed: {normal}");
}

// ---------------------------------------------------------------------------
// 7. fix idempotent — second run is clean
// ---------------------------------------------------------------------------

#[test]
fn lint_fix_idempotent() {
    let (repo, project) = setup_with_term();
    write_doc(&project, "intro", "mindrepo\n");

    // First fix
    mf(&repo).args(["term", "lint", "--fix", "-y", "--project", "alpha"]).assert().code(0);

    // Second fix — should be clean
    let output = mf(&repo).args(["term", "lint", "--fix", "-y", "--project", "alpha"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("No term issues found."), "second run clean: {stdout}");
}

// ---------------------------------------------------------------------------
// 8. JSON envelope for --fix
// ---------------------------------------------------------------------------

#[test]
fn lint_fix_json_shape() {
    let (repo, project) = setup_with_term();
    write_doc(&project, "intro", "mindrepo\n");

    let output =
        mf(&repo).args(["--format", "json", "term", "lint", "--fix", "-y", "--project", "alpha"]).output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["status"], "ok");
    assert!(parsed["data"]["fixed_count"].as_u64().unwrap_or(0) >= 1);
    assert_eq!(parsed["data"]["modified_files"].as_array().unwrap().len(), 1);
    assert!(parsed["data"].get("would_apply_count").is_some(), "report must have would_apply_count");
}

// ═══════════════════════════════════════════════════════════════════════════
// US1 — --fix confirmation gate
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn lint_fix_non_tty_without_yes_exits_2() {
    let (repo, project) = setup_with_term();
    write_doc(&project, "intro", "mindrepo here\n");

    // Test process stdout is not a TTY; --fix without -y must exit 2
    let output = mf(&repo).args(["term", "lint", "--fix", "--project", "alpha"]).output().unwrap();
    assert_eq!(output.status.code(), Some(2), "expected exit 2, got: {:?}", output.status.code());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--yes") || stderr.contains("-y"), "hint missing: {stderr}");
}

#[test]
fn lint_fix_force_flag_bypasses_confirmation() {
    let (repo, project) = setup_with_term();
    write_doc(&project, "intro", "mindrepo here\n");

    mf(&repo).args(["term", "lint", "--fix", "--force", "--project", "alpha"]).assert().code(0);
    let content = fs::read_to_string(project.join("docs/intro.md")).unwrap();
    assert!(content.contains("Mind Repo"));
}

// ═══════════════════════════════════════════════════════════════════════════
// US2 — ASCII word-boundary matching
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn lint_word_boundary_no_false_positives() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    fs::create_dir_all(project.join("docs")).unwrap();
    let index_yaml = r#"schema_version: '1'
terms:
  - term: AI
    corrections:
      - original: ai
        correct: AI
"#;
    common::write_index(&repo, "alpha", index_yaml);
    // "ai" in "training" and "detail" must not match
    write_doc(&project, "doc", "we use ai for training detail\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.matches("→ \"AI\"").count(), 1, "only standalone ai: {stdout}");
}

#[test]
fn lint_word_boundary_no_partial_match() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    fs::create_dir_all(project.join("docs")).unwrap();
    let index_yaml = r#"schema_version: '1'
terms:
  - term: MindRepo
    corrections:
      - original: mindrepo
        correct: MindRepo
"#;
    common::write_index(&repo, "alpha", index_yaml);
    // only standalone "mindrepo" should match
    write_doc(&project, "doc", "the mindrepo, submindrepo, mindreport\n");

    let output = mf(&repo).args(["term", "lint", "--project", "alpha"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.matches("→ \"MindRepo\"").count(), 1, "only standalone: {stdout}");
}

// ═══════════════════════════════════════════════════════════════════════════
// US2 — overlapping corrections don't panic --fix
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn lint_fix_overlapping_corrections_no_panic() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    fs::create_dir_all(project.join("docs")).unwrap();
    // Two corrections whose originals can overlap in the same text:
    // "mini pass" and "pass" — when "mini pass" appears, both match.
    let index_yaml = r#"schema_version: '1'
terms:
  - term: Mini PaaS
    corrections:
      - original: mini pass
        correct: mini PaaS
  - term: PaaS
    corrections:
      - original: pass
        correct: PaaS
"#;
    common::write_index(&repo, "alpha", index_yaml);
    write_doc(&project, "doc", "we use mini pass for deployment\n");

    let output = mf(&repo).args(["term", "lint", "--fix", "-y", "--project", "alpha"]).output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let content = fs::read_to_string(project.join("docs/doc.md")).unwrap();
    // Only the longer match should apply; the nested one is skipped.
    assert!(content.contains("mini PaaS"), "should fix mini pass -> mini PaaS: {content}");
}

#[test]
fn lint_fix_overlapping_corrections_dry_run_no_panic() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    fs::create_dir_all(project.join("docs")).unwrap();
    let index_yaml = r#"schema_version: '1'
terms:
  - term: Mini PaaS
    corrections:
      - original: mini pass
        correct: mini PaaS
  - term: PaaS
    corrections:
      - original: pass
        correct: PaaS
"#;
    common::write_index(&repo, "alpha", index_yaml);
    write_doc(&project, "doc", "mini pass test\n");

    let output = mf(&repo).args(["term", "lint", "--fix", "--dry-run", "--project", "alpha"]).output().unwrap();
    // dry-run exits 1 when there are findings
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("dry-run"), "stdout: {stdout}");
}
