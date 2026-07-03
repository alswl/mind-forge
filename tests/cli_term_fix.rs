use assert_cmd::Command;
use std::fs;

mod common;

fn setup_with_term() -> (common::TempDir, std::path::PathBuf) {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    fs::create_dir_all(project.join("docs")).unwrap();
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
  - term: Other
    aliases: []
    tags: []
    corrections: []
"#;
    common::write_index(&repo, "alpha", index_yaml);
    (repo, project)
}

fn mf(repo: &common::TempDir) -> Command {
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.args(["--root", repo.path().to_str().unwrap()]);
    cmd
}

#[test]
fn exact_pair_and_exclusions_control_applied_corrections() {
    let (repo, project) = setup_with_term();
    common::write_index(
        &repo,
        "alpha",
        r#"schema_version: '1'
terms:
  - term: Alpha
    corrections:
      - original: alpha-one
        correct: Alpha
      - original: alpha-two
        correct: Alpha
  - term: Beta
    corrections:
      - original: beta-one
        correct: Beta
"#,
    );
    fs::write(project.join("docs/select.md"), "alpha-one alpha-two beta-one\n").unwrap();
    let output = mf(&repo)
        .args([
            "term",
            "fix",
            "--project",
            "alpha",
            "docs/select.md",
            "--term",
            "Alpha:alpha-one",
            "--exclude-original",
            "alpha-two",
            "--exclude-term",
            "Beta",
            "-y",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(fs::read_to_string(project.join("docs/select.md")).unwrap(), "Alpha alpha-two beta-one\n");
}

#[test]
fn malformed_qualified_selector_returns_usage_error_json() {
    let (repo, _) = setup_with_term();
    let output = mf(&repo)
        .args(["--output", "json", "term", "fix", "--project", "alpha", "--term", "Mind Repo:", "-y"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let value: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(value["error"]["kind"], "usage");
}
// ---------------------------------------------------------------------------
// 1. replace definition
// ---------------------------------------------------------------------------

#[test]
fn fix_term_replace_definition() {
    let (repo, _project) = setup_with_term();
    let output = mf(&repo)
        .args(["term", "update", "Mind Repo", "--definition", "new definition", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("✓ updated term:"), "stdout: {stdout}");

    // Verify index
    let index = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert!(index.contains("new definition"), "index: {index}");
}

// ---------------------------------------------------------------------------
// 2. append alias and tag
// ---------------------------------------------------------------------------

#[test]
fn fix_term_append_alias_and_tag() {
    let (repo, _project) = setup_with_term();
    let output = mf(&repo)
        .args(["term", "update", "Mind Repo", "--alias", "new-alias", "--tag", "new-tag", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("✓ updated term: Mind Repo (2 fields)"), "stdout: {stdout}");

    let index = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert!(index.contains("new-alias"), "alias added: {index}");
    assert!(index.contains("new-tag"), "tag added: {index}");
}

// ---------------------------------------------------------------------------
// 3. combined changes atomic
// ---------------------------------------------------------------------------

#[test]
fn fix_term_combined_changes_atomic() {
    let (repo, _project) = setup_with_term();
    let output = mf(&repo)
        .args([
            "term",
            "update",
            "Other",
            "--definition",
            "combined",
            "--alias",
            "new-a",
            "--tag",
            "new-t",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());

    let index = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert!(index.contains("combined"));
    assert!(index.contains("new-a"));
    assert!(index.contains("new-t"));
}

// ---------------------------------------------------------------------------
// 4. existing alias silently deduped
// ---------------------------------------------------------------------------

#[test]
fn fix_term_silently_ignores_existing_alias() {
    let (repo, _project) = setup_with_term();
    // mr already exists as alias
    let output =
        mf(&repo).args(["term", "update", "Mind Repo", "--alias", "mr", "--project", "alpha"]).output().unwrap();

    assert!(output.status.success());
    // The CLI reports argument count, but the service deduplicates.
    // Verify index still has only 1 alias entry.
    let index = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert_eq!(index.matches("- mr").count(), 1, "no duplicate alias: {index}");
}

// ---------------------------------------------------------------------------
// 5. term not found → usage
// ---------------------------------------------------------------------------

#[test]
fn fix_term_not_found_rejected() {
    let (repo, _project) = setup_with_term();
    let output =
        mf(&repo).args(["term", "update", "NonExistent", "--definition", "x", "--project", "alpha"]).output().unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not found"), "stderr: {stderr}");
}

// ---------------------------------------------------------------------------
// 6. no change flags → usage
// ---------------------------------------------------------------------------

#[test]
fn fix_term_no_change_flags_rejected() {
    let (repo, _project) = setup_with_term();
    let output = mf(&repo).args(["term", "update", "Mind Repo", "--project", "alpha"]).output().unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("at least one"), "stderr: {stderr}");
}

// ---------------------------------------------------------------------------
// 7. corrections unchanged
// ---------------------------------------------------------------------------

#[test]
fn fix_term_does_not_touch_corrections() {
    let (repo, _project) = setup_with_term();
    // Mind Repo has 1 correction. After fix, should still have 1.
    mf(&repo).args(["term", "update", "Mind Repo", "--definition", "updated", "--project", "alpha"]).assert().code(0);

    let index = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    // Count corrections (original: mindrepo should be there once)
    assert!(index.contains("original: mindrepo"));
}

// ---------------------------------------------------------------------------
// 8. JSON shape
// ---------------------------------------------------------------------------

#[test]
fn fix_term_json_shape() {
    let (repo, _project) = setup_with_term();
    let output = mf(&repo)
        .args(["--output", "json", "term", "update", "Mind Repo", "--definition", "json-test", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["data"]["kind"], "term");
    assert_eq!(parsed["data"]["identity"], "Mind Repo");
    assert!(parsed["data"]["details"]["changes"].is_object());
    assert!(!parsed["data"]["dry_run"].as_bool().unwrap());
}

// T052
#[test]
fn fix_schema_version_unchanged() {
    let repo = common::setup_repo();
    // Write a schema-version global terms file
    let schema_yaml = "schema_version: '1'\nterms:\n  - term: cafed\n    definition: null\n    aliases: []\n    tags: []\n    corrections: []\n";
    std::fs::write(repo.path().join("minds-terms.yaml"), schema_yaml).unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "update", "cafed", "--definition", "the cafed thing"])
        .output()
        .unwrap();

    assert!(output.status.success(), "exit 0: {:?}", output.status.code());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("✓ updated term:"), "stdout: {stdout}");

    let content = fs::read_to_string(repo.path().join("minds-terms.yaml")).unwrap();
    assert!(content.contains("the cafed thing"), "definition updated: {content}");
    assert!(content.contains("schema_version"), "must remain schema-version: {content}");
}

// --boundary flag (spec 052) — end-to-end round-trip via `term correction update`.

#[test]
fn fix_term_correction_boundary_flag_round_trip() {
    let (repo, project) = setup_with_term();
    let output = mf(&repo)
        .args([
            "term",
            "correction",
            "update",
            "Mind Repo",
            "mindrepo",
            "--boundary",
            "standalone",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "stderr: {:?}", String::from_utf8_lossy(&output.stderr));

    let index = fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(!index.contains("boundary:"), "standalone is default and must not be serialized: {index}");

    // Flipping to loose must write the field (loose is now explicit, not default).
    let output = mf(&repo)
        .args(["term", "correction", "update", "Mind Repo", "mindrepo", "--boundary", "loose", "--project", "alpha"])
        .output()
        .unwrap();
    assert!(output.status.success(), "stderr: {:?}", String::from_utf8_lossy(&output.stderr));

    let index = fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index.contains("boundary: loose"), "loose is explicit and must be serialized: {index}");
}

#[test]
fn fix_term_correction_boundary_invalid_value_rejected() {
    let (repo, _project) = setup_with_term();
    let output = mf(&repo)
        .args(["term", "correction", "update", "Mind Repo", "mindrepo", "--boundary", "bogus", "--project", "alpha"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("invalid boundary"), "stderr: {stderr}");
}

// ── Spec 053: --term filter ──────────────────────────────────────────────────

/// Set up a project with two terms (RAG, LLM) each having corrections, and a
/// document containing both variants.
fn setup_two_term_fixture() -> (common::TempDir, std::path::PathBuf) {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    fs::create_dir_all(project.join("docs")).unwrap();
    let index_yaml = r#"schema_version: '1'
terms:
  - term: RAG
    definition: Retrieval Augmented Generation
    aliases: []
    tags: []
    corrections:
      - original: rag
        correct: RAG
  - term: LLM
    definition: Large Language Model
    aliases: []
    tags: []
    corrections:
      - original: llm
        correct: LLM
"#;
    common::write_index(&repo, "alpha", index_yaml);
    // Create both project and global versions for tests that don't need project scope
    let mut mf_cmd = Command::cargo_bin("mf").unwrap();
    mf_cmd.args(["--root", repo.path().to_str().unwrap(), "term", "lint", "--project", "alpha", "--json"]);
    // Trigger index load but don't care about output — just verify the setup worked
    common::write_doc(&repo, "alpha", "intro", "rag is better than llm");
    (repo, project)
}

/// T008: --term RAG applies only RAG corrections, LLM variant unchanged.
#[test]
fn term_filter_single_term_project_scope() {
    let (repo, project) = setup_two_term_fixture();
    let doc_path = project.join("docs").join("intro.md");

    // Verify initial file content
    let content = fs::read_to_string(&doc_path).unwrap();
    assert!(content.contains("rag"), "precondition: doc contains 'rag'");
    assert!(content.contains("llm"), "precondition: doc contains 'llm'");

    // Fix only RAG
    let output = mf(&repo).args(["term", "fix", "--project", "alpha", "--term", "RAG", "--yes"]).output().unwrap();

    assert!(output.status.success(), "exit 0: stderr={}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("scoped to term(s): RAG"), "stdout: {stdout}");

    // RAG should be fixed, LLM should remain unchanged
    let fixed = fs::read_to_string(&doc_path).unwrap();
    assert!(!fixed.contains("rag"), "rag should be fixed: {fixed}");
    assert!(fixed.contains("llm"), "llm should remain unchanged: {fixed}");
}

/// T009: --term filter works in global scope.
#[test]
fn term_filter_single_term_global_scope() {
    let repo = common::setup_repo();
    // Write global terms
    let global_terms = r#"schema_version: '1'
terms:
  - term: RAG
    definition: null
    aliases: []
    tags: []
    corrections:
      - original: rag
        correct: RAG
  - term: LLM
    definition: null
    aliases: []
    tags: []
    corrections:
      - original: llm
        correct: LLM
"#;
    fs::write(repo.path().join("minds-terms.yaml"), global_terms).unwrap();
    fs::write(repo.path().join("test.md"), "use rag over llm").unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "fix", "test.md", "--term", "RAG", "--yes"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "exit code={:?} stderr={} stdout={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );

    let fixed = fs::read_to_string(repo.path().join("test.md")).unwrap();
    assert!(!fixed.contains("rag"), "rag should be fixed: {fixed}");
    assert!(fixed.contains("llm"), "llm should remain unchanged: {fixed}");
}

/// T010: --term with --dry-run marks only targeted term as selected.
#[test]
fn term_filter_dry_run_reports_only_targeted() {
    let (repo, _project) = setup_two_term_fixture();
    let doc_path = _project.join("docs").join("intro.md");
    let original = fs::read_to_string(&doc_path).unwrap();

    let output =
        mf(&repo).args(["term", "lint", "--fix", "--dry-run", "--term", "RAG", "--project", "alpha"]).output().unwrap();

    // --fix --dry-run exits 1 when findings exist (lint semantics)
    assert_eq!(
        output.status.code(),
        Some(1),
        "exit 1 with findings: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    // The full preview remains reviewable, but only RAG is selected.
    assert!(stdout.contains("[RAG]") && stdout.contains("selection=selected"), "stdout: {stdout}");
    assert!(stdout.contains("[LLM]") && stdout.contains("selection=not_selected"), "stdout: {stdout}");
    assert!(stdout.contains("scoped to term(s): RAG"), "stdout: {stdout}");
    // Should NOT touch the file
    let after = fs::read_to_string(&doc_path).unwrap();
    assert_eq!(original, after, "dry-run must not modify file");
}

/// T011: unknown term name exits 2 with diagnostic.
#[test]
fn term_filter_unknown_term_exits_2() {
    let (repo, _project) = setup_two_term_fixture();

    let output = mf(&repo).args(["term", "fix", "--project", "alpha", "--term", "NOPE", "--yes"]).output().unwrap();

    assert_eq!(output.status.code(), Some(2), "exit 2 for unknown term");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("unknown term"), "stderr must name unknown term: {stderr}");
    assert!(stderr.contains("NOPE"), "stderr must contain the unknown name: {stderr}");
}

/// T012: no --term produces same result as before (regression, SC-002).
#[test]
fn term_filter_absent_is_whole_glossary_regression() {
    let (repo, project) = setup_two_term_fixture();
    let doc_path = project.join("docs").join("intro.md");

    let output = mf(&repo).args(["term", "fix", "--project", "alpha", "--yes"]).output().unwrap();

    assert!(output.status.success(), "exit 0: stderr={}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.contains("scoped to term(s):"), "no filter annotation when absent: {stdout}");

    // Both should be fixed
    let fixed = fs::read_to_string(&doc_path).unwrap();
    assert!(!fixed.contains("rag"), "rag should be fixed: {fixed}");
    assert!(!fixed.contains("llm"), "llm should be fixed: {fixed}");
}

/// T013: JSON output includes term_filter array; absent when no filter.
#[test]
fn term_filter_json_envelope() {
    let (repo, _project) = setup_two_term_fixture();

    // With filter — exit 1 because findings exist (lint semantics for --fix --dry-run)
    let output = mf(&repo)
        .args(["--json", "term", "lint", "--fix", "--dry-run", "--term", "RAG", "--project", "alpha"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1), "exit 1 with findings");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let term_filter = parsed["data"]["term_filter"].as_array().unwrap();
    assert_eq!(term_filter.len(), 1);
    assert_eq!(term_filter[0].as_str().unwrap(), "RAG");

    // Without filter — also exit 1 (findings)
    let output =
        mf(&repo).args(["--json", "term", "lint", "--fix", "--dry-run", "--project", "alpha"]).output().unwrap();

    assert_eq!(output.status.code(), Some(1), "exit 1 with findings");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let term_filter = parsed["data"]["term_filter"].as_array().unwrap();
    assert!(term_filter.is_empty(), "term_filter must be [] when absent: {stdout}");
}

// ── US2: Multi-term filter ──────────────────────────────────────────────────

/// T019: --term RAG --term LLM applies both, leaves TPU untouched.
#[test]
fn term_filter_multi_term_union() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    fs::create_dir_all(project.join("docs")).unwrap();
    let index_yaml = r#"schema_version: '1'
terms:
  - term: RAG
    definition: null
    aliases: []
    tags: []
    corrections:
      - original: rag
        correct: RAG
  - term: LLM
    definition: null
    aliases: []
    tags: []
    corrections:
      - original: llm
        correct: LLM
  - term: TPU
    definition: null
    aliases: []
    tags: []
    corrections:
      - original: tpu
        correct: TPU
"#;
    common::write_index(&repo, "alpha", index_yaml);
    common::write_doc(&repo, "alpha", "intro", "use rag and llm not tpu");
    let doc_path = project.join("docs").join("intro.md");

    let output = mf(&repo)
        .args(["term", "fix", "--project", "alpha", "--term", "RAG", "--term", "LLM", "--yes"])
        .output()
        .unwrap();

    assert!(output.status.success(), "exit 0: stderr={}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("scoped to term(s): RAG, LLM"), "stdout: {stdout}");

    let fixed = fs::read_to_string(&doc_path).unwrap();
    assert!(!fixed.contains("rag"), "rag should be fixed: {fixed}");
    assert!(!fixed.contains("llm"), "llm should be fixed: {fixed}");
    assert!(fixed.contains("tpu"), "tpu should remain unchanged: {fixed}");
}

/// T020: mixed valid+unknown exits 2 with no edits (FR-006 strictness).
#[test]
fn term_filter_mixed_valid_and_unknown_exits_2() {
    let (repo, project) = setup_two_term_fixture();
    let doc_path = project.join("docs").join("intro.md");
    let original = fs::read_to_string(&doc_path).unwrap();

    let output = mf(&repo)
        .args(["term", "fix", "--project", "alpha", "--term", "RAG", "--term", "NOPE", "--yes"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2), "exit 2 for mixed valid+unknown");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("NOPE"), "stderr must contain unknown name: {stderr}");
    assert!(stderr.contains("unknown term"), "stderr: {stderr}");

    // No edits
    let after = fs::read_to_string(&doc_path).unwrap();
    assert_eq!(original, after, "file must not be modified on error");
}

/// T021: JSON term_filter preserves all requested names for multi-term.
#[test]
fn term_filter_multi_term_json() {
    let (repo, _project) = setup_two_term_fixture();

    let output = mf(&repo)
        .args(["--json", "term", "lint", "--fix", "--dry-run", "--term", "RAG", "--term", "LLM", "--project", "alpha"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1), "exit 1 with findings");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let term_filter = parsed["data"]["term_filter"].as_array().unwrap();
    assert_eq!(term_filter.len(), 2);
    let names: Vec<&str> = term_filter.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(names.contains(&"RAG"));
    assert!(names.contains(&"LLM"));
}
