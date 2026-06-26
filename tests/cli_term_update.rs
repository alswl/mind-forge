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

test_rejected_flag!(update_rejects_delete_correction_flag, "--delete-correction", "rag");
test_rejected_flag!(update_rejects_correction_match_flag, "--correction-match", "rag:substring");
test_rejected_flag!(update_rejects_correction_fix_flag, "--correction-fix", "rag:suggested");
test_rejected_flag!(update_rejects_correction_pinyin_flag, "--correction-pinyin", "rag:ni hao");
test_rejected_flag!(update_rejects_correction_boundary_flag, "--correction-boundary", "rag:loose");

// Global scope also rejects removed flags
#[test]
fn update_global_rejects_delete_correction_flag() {
    let (repo, _project) = setup_with_term();
    let global_before = fs::read_to_string(repo.path().join("minds-terms.yaml")).unwrap();

    let output = mf(&repo).args(["term", "update", "GlobalTerm", "--delete-correction", "gt"]).output().unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("correction") || stderr.contains("term correction"),
        "stderr should mention `mf term correction`: {stderr}"
    );

    // Global storage must be unchanged
    let global_after = fs::read_to_string(repo.path().join("minds-terms.yaml")).unwrap();
    assert_eq!(global_before, global_after, "global storage must not change on rejection");
}

// ── JSON error envelope for removed flag ─────────────────────────────────

#[test]
fn update_rejects_correction_flag_json_error_envelope() {
    let (repo, _project) = setup_with_term();

    let output = mf(&repo)
        .args(["--json", "term", "update", "RAG", "--correction-fix", "rag:suggested", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stderr).expect("valid JSON on stderr");
    assert_eq!(v["status"], "error", "JSON error envelope: {stderr}");
    assert_eq!(v["error"]["kind"], "usage", "error kind should be usage: {stderr}");
    assert!(
        v["error"]["message"].as_str().unwrap_or("").contains("correction"),
        "error message should mention correction: {stderr}"
    );
}

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
