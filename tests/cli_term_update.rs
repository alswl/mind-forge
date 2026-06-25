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

// ── T013: Reject missing correction-fix target ───────────────────────────

#[test]
fn update_rejects_missing_correction_fix() {
    let (repo, _project) = setup_with_term();
    let index_before = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();

    let output = mf(&repo)
        .args(["term", "update", "RAG", "--correction-fix", "missing:suggested", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("missing"), "stderr: {stderr}");
    assert!(stderr.contains("not found"), "stderr: {stderr}");

    // Storage must be unchanged
    let index_after = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert_eq!(index_before, index_after, "storage must not change on rejection");
}

// ── T014: Reject missing correction-match, pinyin, boundary targets ──────

#[test]
fn update_rejects_missing_correction_match() {
    let (repo, _project) = setup_with_term();
    let index_before = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();

    let output = mf(&repo)
        .args(["term", "update", "RAG", "--correction-match", "missing:word", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("missing"), "stderr: {stderr}");

    let index_after = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert_eq!(index_before, index_after);
}

#[test]
fn update_rejects_missing_correction_pinyin() {
    let (repo, _project) = setup_with_term();
    let index_before = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();

    let output = mf(&repo)
        .args(["term", "update", "RAG", "--correction-pinyin", "missing:x", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("missing"), "stderr: {stderr}");

    let index_after = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert_eq!(index_before, index_after);
}

#[test]
fn update_rejects_missing_correction_boundary() {
    let (repo, _project) = setup_with_term();
    let index_before = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();

    let output = mf(&repo)
        .args(["term", "update", "RAG", "--correction-boundary", "missing:loose", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("missing"), "stderr: {stderr}");

    let index_after = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert_eq!(index_before, index_after);
}

// ── T015: Preserve valid existing correction attribute updates ───────────

#[test]
fn update_preserves_valid_correction_attribute_update() {
    let (repo, _project) = setup_with_term();

    let output = mf(&repo)
        .args(["term", "update", "RAG", "--correction-fix", "rag:suggested", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("updated term"), "stdout: {stdout}");

    // Verify the fix kind was changed
    let index = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert!(index.contains("fix: suggested"), "index should reflect suggested fix: {index}");
}

#[test]
fn update_preserves_valid_correction_match_update() {
    let (repo, _project) = setup_with_term();

    let output = mf(&repo)
        .args(["term", "update", "RAG", "--correction-match", "rag:substring", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("updated term"), "stdout: {stdout}");

    let index = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert!(index.contains("match: substring"), "index should reflect substring match: {index}");
}

// ── T016: JSON error-envelope for rejected missing correction update ─────

#[test]
fn update_missing_correction_json_error_envelope() {
    let (repo, _project) = setup_with_term();

    let output = mf(&repo)
        .args(["--json", "term", "update", "RAG", "--correction-fix", "missing:suggested", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    // JSON errors are rendered to stderr
    let v: serde_json::Value = serde_json::from_str(&stderr).expect("valid JSON on stderr");
    assert_eq!(v["status"], "error", "JSON error envelope: {stderr}");
    assert!(v["error"]["message"].as_str().unwrap_or("").contains("missing"), "error message: {stderr}");
}

// ── T017: Reject --misrecognition on term update ────────────────────────

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

// ── T018: Project/global parity for rejected missing correction updates ──

#[test]
fn update_global_rejects_missing_correction_fix() {
    let (repo, _project) = setup_with_term();

    // Read global terms before
    let global_before = fs::read_to_string(repo.path().join("minds-terms.yaml")).unwrap();

    let output =
        mf(&repo).args(["term", "update", "GlobalTerm", "--correction-fix", "missing:suggested"]).output().unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("missing"), "stderr: {stderr}");

    // Global storage must be unchanged
    let global_after = fs::read_to_string(repo.path().join("minds-terms.yaml")).unwrap();
    assert_eq!(global_before, global_after, "global storage must not change on rejection");
}

#[test]
fn update_global_preserves_valid_correction_update() {
    let (repo, _project) = setup_with_term();

    let output = mf(&repo).args(["term", "update", "GlobalTerm", "--correction-fix", "gt:suggested"]).output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("updated term"), "stdout: {stdout}");

    let global_after = fs::read_to_string(repo.path().join("minds-terms.yaml")).unwrap();
    assert!(global_after.contains("fix: suggested"), "global should reflect suggested fix: {global_after}");
}

// ════════════════════════════════════════════════════════════════════════════
// US2: Dry-run preview for term update
// ════════════════════════════════════════════════════════════════════════════

// ── T025: Text dry-run for --tag with unchanged storage ──────────────────

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

// ── T026: JSON dry-run envelope for --description --dry-run ───────────────

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

// ── T027: Dry-run for --delete-alias, --delete-tag, --delete-correction ──

#[test]
fn update_dry_run_delete_flags() {
    let (repo, _project) = setup_with_term();
    let index_before = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();

    let output = mf(&repo)
        .args(["term", "update", "RAG", "--delete-correction", "rag", "--dry-run", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("dry-run") || stdout.contains("[dry-run]"), "stdout: {stdout}");

    let index_after = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert_eq!(index_before, index_after, "dry-run must not modify storage");
}

// ── T028: Dry-run for correction attribute changes with unchanged storage ─

#[test]
fn update_dry_run_correction_attr_changes() {
    let (repo, _project) = setup_with_term();
    let index_before = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();

    let output = mf(&repo)
        .args([
            "term",
            "update",
            "RAG",
            "--correction-match",
            "rag:substring",
            "--correction-fix",
            "ragg:required",
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

// ── T029: Dry-run global fallthrough warning ─────────────────────────────

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
    // Should warn about global fallback
    assert!(stderr.contains("WARN") || stderr.contains("global"), "stderr should warn: {stderr}");
}
