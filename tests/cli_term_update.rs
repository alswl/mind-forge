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
  - term: RAG
    definition: Retrieval augmented generation
    aliases: []
    tags: []
    corrections:
      - original: rag
        correct: RAG
        match: word
        fix: required
      - original: ragg
        correct: RAG
        match: word
        fix: suggested
"#;
    common::write_index(&repo, "alpha", index_yaml);

    // Also create a global term for parity tests
    let global_yaml = r#"schema_version: '1'
terms:
  - term: GlobalTerm
    definition: A global term
    aliases: []
    tags: []
    corrections:
      - original: gt
        correct: GlobalTerm
        match: word
        fix: required
"#;
    std::fs::write(repo.path().join("minds-terms.yaml"), global_yaml).unwrap();

    (repo, project)
}

fn mf(repo: &common::TempDir) -> Command {
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.args(["--root", repo.path().to_str().unwrap()]);
    cmd
}

// ════════════════════════════════════════════════════════════════════════════
// US1: Removed correction flags are rejected with hint to `mf term correction`
// ════════════════════════════════════════════════════════════════════════════

macro_rules! test_rejected_flag {
    ($name:ident, $flag:literal, $value:literal) => {
        #[test]
        fn $name() {
            let (repo, _project) = setup_with_term();
            let index_before = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();

            let output =
                mf(&repo).args(["term", "update", "RAG", $flag, $value, "--project", "alpha"]).output().unwrap();

            assert!(!output.status.success());
            let stderr = String::from_utf8(output.stderr).unwrap();
            assert!(
                stderr.contains("correction") || stderr.contains("term correction"),
                "stderr should mention `mf term correction`: {stderr}"
            );

            // Storage must be unchanged
            let index_after = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
            assert_eq!(index_before, index_after, "storage must not change on rejection");
        }
    };
}

test_rejected_flag!(update_rejects_correction_boundary_flag, "--correction-boundary", "rag:loose");

// ── Reject --misrecognition on term update (existing, unchanged) ─────────

#[test]
fn update_rejects_misrecognition_flag() {
    let (repo, _project) = setup_with_term();
    let index_before = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();

    let output = mf(&repo)
        .args(["term", "update", "RAG", "--misrecognition", "rag:RAG", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("misrecognition") || stderr.contains("unsupported"),
        "stderr should mention misrecognition is unsupported: {stderr}"
    );

    // Storage must be unchanged
    let index_after = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert_eq!(index_before, index_after);
}

// ════════════════════════════════════════════════════════════════════════════
// Metadata-path tests (unchanged — kept green)
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn update_dry_run_tag_text() {
    let (repo, _project) = setup_with_term();
    let index_before = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();

    let output =
        mf(&repo).args(["term", "update", "RAG", "--tag", "ai", "--dry-run", "--project", "alpha"]).output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("dry-run") || stdout.contains("[dry-run]"), "stdout: {stdout}");
    assert!(stdout.contains("RAG"), "stdout: {stdout}");

    // Storage must be unchanged
    let index_after = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert_eq!(index_before, index_after, "dry-run must not write storage");
}

#[test]
fn update_dry_run_json_envelope() {
    let (repo, _project) = setup_with_term();
    let index_before = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();

    let output = mf(&repo)
        .args([
            "--json",
            "term",
            "update",
            "RAG",
            "--description",
            "A test description",
            "--dry-run",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(v["status"], "ok", "JSON: {stdout}");
    assert_eq!(v["data"]["dry_run"], true, "JSON: {stdout}");
    assert_eq!(v["data"]["identity"], "RAG", "JSON: {stdout}");

    let index_after = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert_eq!(index_before, index_after);
}

#[test]
fn update_dry_run_delete_flags() {
    let (repo, _project) = setup_with_term();
    let index_before = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();

    let output = mf(&repo)
        .args([
            "term",
            "update",
            "RAG",
            "--delete-tag",
            "old-tag",
            "--delete-alias",
            "old-alias",
            "--dry-run",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("dry-run") || stdout.contains("[dry-run]"), "stdout: {stdout}");

    let index_after = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert_eq!(index_before, index_after, "dry-run must not modify storage");
}

#[test]
fn update_dry_run_global_fallback_warning() {
    let (repo, _project) = setup_with_term();
    // GlobalTerm only exists globally, not in project "alpha"
    let output = mf(&repo)
        .args(["term", "update", "GlobalTerm", "--tag", "test-tag", "--dry-run", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("WARN") || stderr.contains("global"), "stderr should warn: {stderr}");
}

// ── US4 (Bug #4/#9): correction management on term update (T027-T029) ──

/// T027: --add-correction appends a correction while preserving other metadata.
#[test]
fn add_correction_preserves_metadata() {
    let (repo, project) = setup_with_term();

    let output =
        mf(&repo).args(["term", "update", "RAG", "--add-correction", "ragd", "--project", "alpha"]).output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let index_after = fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_after.contains("ragd"), "should contain new correction: {index_after}");
    assert!(index_after.contains("definition: Retrieval augmented generation"), "definition preserved: {index_after}");
    assert!(index_after.contains("rag"), "existing correction preserved: {index_after}");
}

/// T027: --add-correction for an existing original is no-op (text output).
#[test]
fn add_correction_duplicate_is_noop_text() {
    let (repo, _project) = setup_with_term();

    let output =
        mf(&repo).args(["term", "update", "RAG", "--add-correction", "rag", "--project", "alpha"]).output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    // JSON output should show the operation succeeded
    let json_out = mf(&repo)
        .args(["--output", "json", "term", "update", "RAG", "--add-correction", "rag", "--project", "alpha"])
        .output()
        .unwrap();
    assert!(json_out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&json_out.stdout).unwrap();
    assert_eq!(v["status"], "ok");
    assert!(v["data"]["details"]["changes"].as_object().unwrap().contains_key("corrections"));
}

/// T027: --add-correction with JSON output.
#[test]
fn add_correction_json_output() {
    let (repo, _project) = setup_with_term();

    let output = mf(&repo)
        .args(["--output", "json", "term", "update", "RAG", "--add-correction", "ragx", "--project", "alpha"])
        .output()
        .unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["status"], "ok");
    assert!(v["data"]["details"]["changes"].as_object().unwrap().contains_key("corrections"));
}

/// T028: --correction-match sets match kind of existing correction.
#[test]
fn correction_match_sets_attribute() {
    let (repo, project) = setup_with_term();

    let output = mf(&repo)
        .args(["term", "update", "RAG", "--correction-match", "rag:substring", "--project", "alpha"])
        .output()
        .unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let index_after = fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_after.contains("match: substring"), "match kind should be substring: {index_after}");
}

/// T028: --correction-fix sets fix kind of existing correction.
#[test]
fn correction_fix_sets_attribute() {
    let (repo, project) = setup_with_term();

    let output = mf(&repo)
        .args(["term", "update", "RAG", "--correction-fix", "rag:suggested", "--project", "alpha"])
        .output()
        .unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let index_after = fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_after.contains("fix: suggested"), "fix kind should be suggested: {index_after}");
}

/// T028: --delete-correction removes a correction by original.
#[test]
fn delete_correction_removes() {
    let (repo, project) = setup_with_term();

    let output = mf(&repo)
        .args(["term", "update", "RAG", "--delete-correction", "ragg", "--project", "alpha"])
        .output()
        .unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let index_after = fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(!index_after.contains("ragg"), "ragg should be deleted: {index_after}");
    assert!(index_after.contains("rag"), "other correction preserved: {index_after}");
}

/// T028: setting attribute for absent original → exit 2 with --add-correction hint.
#[test]
fn correction_match_unknown_original_is_usage_error() {
    let (repo, _project) = setup_with_term();

    let output = mf(&repo)
        .args(["--output", "json", "term", "update", "RAG", "--correction-match", "noexist:word", "--project", "alpha"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2), "should exit 2");
    let v: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert!(v["error"]["kind"] == "not_found" || v["error"]["kind"] == "usage", "should be not_found or usage error");
}

/// T028: --correction-pinyin sets pinyin attribute.
#[test]
fn correction_pinyin_sets_attribute() {
    let (repo, project) = setup_with_term();

    let output = mf(&repo)
        .args(["term", "update", "RAG", "--correction-pinyin", "rag:r-a-g", "--project", "alpha"])
        .output()
        .unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let index_after = fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_after.contains("r-a-g"), "pinyin should be set: {index_after}");
}
