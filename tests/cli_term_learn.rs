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
      - mindrepo
    tags:
      - infra
    corrections: []
  - term: Other
    aliases:
      - other-alias
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

// ---------------------------------------------------------------------------
// 1. learn basic append — main name hit
// ---------------------------------------------------------------------------

#[test]
fn learn_basic_append() {
    let (repo, _project) = setup_with_term();
    let output = mf(&repo)
        .args(["term", "learn", "--original", "old-mindrepo", "--correct", "Mind Repo", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("learned"), "stdout: {stdout}");

    // Verify index was updated
    let index = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert!(index.contains("old-mindrepo"), "correction added: {index}");
}

// ---------------------------------------------------------------------------
// 2. learn idempotent — same pair again
// ---------------------------------------------------------------------------

#[test]
fn learn_idempotent_when_pair_exists() {
    let (repo, _project) = setup_with_term();
    // First — should succeed
    mf(&repo)
        .args(["term", "learn", "--original", "mr-old", "--correct", "Mind Repo", "--project", "alpha"])
        .assert()
        .code(0);

    // Second — should be idempotent
    let output = mf(&repo)
        .args(["term", "learn", "--original", "mr-old", "--correct", "Mind Repo", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("already exists"), "idempotent: {stdout}");
}

// ---------------------------------------------------------------------------
// 3. correct unknown → usage
// ---------------------------------------------------------------------------

#[test]
fn learn_correct_unknown_rejected() {
    let (repo, _project) = setup_with_term();
    let output = mf(&repo)
        .args(["term", "learn", "--original", "foo", "--correct", "NonExistent", "--project", "alpha"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("no term registers"), "stderr: {stderr}");
}

// ---------------------------------------------------------------------------
// 4. correct ambiguous → usage
// ---------------------------------------------------------------------------

#[test]
fn learn_correct_ambiguous_rejected() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    fs::create_dir_all(project.join("docs")).unwrap();
    // Two terms both claiming the same alias
    let index_yaml = r#"schema_version: '1'
terms:
  - term: Alpha
    aliases:
      - shared
    corrections: []
  - term: Beta
    aliases:
      - shared
    corrections: []
"#;
    common::write_index(&repo, "alpha", index_yaml);

    let output = mf(&repo)
        .args(["term", "learn", "--original", "foo", "--correct", "shared", "--project", "alpha"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("multiple terms claim"), "stderr: {stderr}");
}

// ---------------------------------------------------------------------------
// 5. original equals term or alias → usage
// ---------------------------------------------------------------------------

#[test]
fn learn_original_equals_term_or_alias_rejected() {
    let (repo, _project) = setup_with_term();
    // original equals term name
    let output = mf(&repo)
        .args(["term", "learn", "--original", "Mind Repo", "--correct", "Mind Repo", "--project", "alpha"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("already a recognized form"), "stderr: {stderr}");

    // original equals alias
    let output2 = mf(&repo)
        .args(["term", "learn", "--original", "mr", "--correct", "Mind Repo", "--project", "alpha"])
        .output()
        .unwrap();

    assert_eq!(output2.status.code(), Some(2));
    let stderr2 = String::from_utf8(output2.stderr).unwrap();
    assert!(stderr2.contains("already a recognized form"), "stderr2: {stderr2}");
}

// ---------------------------------------------------------------------------
// 6. empty original or correct → usage
// ---------------------------------------------------------------------------

#[test]
fn learn_empty_original_or_correct_rejected() {
    let (repo, _project) = setup_with_term();
    // Empty original is a clap error? No — it's a flag, so it passes clap.
    // The empty check is in the service.
    let output =
        mf(&repo).args(["term", "learn", "--alias", "", "--term", "Mind Repo", "--project", "alpha"]).output().unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("requires"), "stderr: {stderr}");
}

// ---------------------------------------------------------------------------
// 7. JSON shape
// ---------------------------------------------------------------------------

#[test]
fn learn_json_shape() {
    let (repo, _project) = setup_with_term();
    let output = mf(&repo)
        .args([
            "--format",
            "json",
            "term",
            "learn",
            "--original",
            "old-term",
            "--correct",
            "Mind Repo",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["data"]["term"], "Mind Repo");
    assert!(!parsed["data"]["corrections"].as_array().unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// 8. --project alpha
// ---------------------------------------------------------------------------

#[test]
fn learn_with_project_root_flags() {
    let (repo, _project) = setup_with_term();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "term",
            "learn",
            "--original",
            "learned-term",
            "--correct",
            "Mind Repo",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
}

// ── US3: Repository-format learn ───────────────────────────────────────────

fn repo_format_fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/term_repo_format").join(name),
    )
    .unwrap()
}

fn write_global_terms(repo: &common::TempDir, content: &str) {
    std::fs::write(repo.path().join("minds-terms.yaml"), content).unwrap();
}

// T038
#[test]
fn learn_appends_misrecognition_repo_format() {
    let repo = common::setup_repo();
    let fixture = repo_format_fixture("simple.yaml");
    write_global_terms(&repo, &fixture);

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "learn", "--term", "cafed", "--alias", "卡维地"])
        .output()
        .unwrap();

    assert!(output.status.success(), "exit 0: {:?}", output.status.code());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("learned"), "stdout: {stdout}");

    let content = fs::read_to_string(repo.path().join("minds-terms.yaml")).unwrap();
    assert!(content.contains("卡维地"), "should contain new misrecognition: {content}");
    // Original entries intact
    assert!(content.contains("凯飞迪"), "original misrecognition intact: {content}");
}

#[test]
fn learn_appends_misrecognition_schema_tagged_repo_format() {
    let repo = common::setup_repo();
    let fixture = "# 会议纪要术语校准表\nschema_version: '1'\n\ncafed:\n  misrecognitions:\n    - 凯飞迪\n";
    write_global_terms(&repo, fixture);

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "learn", "--term", "cafed", "--alias", "caféd"])
        .output()
        .unwrap();

    assert!(output.status.success(), "exit 0: {:?}", output.status.code());
    let content = fs::read_to_string(repo.path().join("minds-terms.yaml")).unwrap();
    assert!(content.starts_with("# 会议纪要术语校准表\nschema_version: '1'\n"), "schema header preserved: {content}");
    assert!(content.contains("    - 凯飞迪\n    - caféd\n"), "misrecognition appended: {content}");
    assert!(!content.contains("terms:"), "must not migrate to schema-version terms list: {content}");
}

// T039
#[test]
fn learn_is_idempotent_repo_format() {
    let repo = common::setup_repo();
    let fixture = repo_format_fixture("simple.yaml");
    write_global_terms(&repo, &fixture);

    // First call
    let output1 = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "learn", "--term", "cafed", "--alias", "卡维地"])
        .output()
        .unwrap();
    assert!(output1.status.success());

    // Second call — idempotent
    let output2 = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "learn", "--term", "cafed", "--alias", "卡维地"])
        .output()
        .unwrap();

    assert!(output2.status.success());
    let stdout2 = String::from_utf8(output2.stdout).unwrap();
    assert!(stdout2.contains("already exists"), "idempotent: {stdout2}");

    let after = fs::read_to_string(repo.path().join("minds-terms.yaml")).unwrap();
    // 卡维地 should only appear once
    assert_eq!(after.matches("卡维地").count(), 1, "should appear exactly once in file: {after}");
}

// T040
#[test]
fn learn_unknown_term_rejected_repo_format() {
    let repo = common::setup_repo();
    let fixture = repo_format_fixture("simple.yaml");
    write_global_terms(&repo, &fixture);

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "learn", "--term", "unknown", "--alias", "foo"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2), "exit 2 for unknown term");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("no term registers"), "stderr: {stderr}");

    let after = fs::read_to_string(repo.path().join("minds-terms.yaml")).unwrap();
    assert_eq!(fixture, after, "file unchanged");
}

// T041
#[test]
fn learn_10_calls_in_order_repo_format() {
    let repo = common::setup_repo();
    write_global_terms(&repo, &repo_format_fixture("simple.yaml"));

    let aliases: [&str; 10] = ["a1", "a2", "a3", "a4", "a5", "a6", "a7", "a8", "a9", "a10"];

    for alias in &aliases {
        let output = Command::cargo_bin("mf")
            .unwrap()
            .args(["--root", repo.path().to_str().unwrap(), "term", "learn", "--term", "cafed", "--alias", alias])
            .output()
            .unwrap();
        assert!(output.status.success(), "learn {}: {:?}", alias, output.status.code());
    }

    let content = fs::read_to_string(repo.path().join("minds-terms.yaml")).unwrap();
    // Verify insertion order: all 10 aliases appear after the original 2
    let orig_pos = content.find("凯飞迪").unwrap();
    for alias in &aliases {
        let pos = content.find(alias).unwrap();
        assert!(pos > orig_pos, "alias {alias} should be after original misrecognitions");
    }
}

// T042
#[test]
fn learn_preserves_other_entries_repo_format() {
    let repo = common::setup_repo();
    let fixture = repo_format_fixture("simple.yaml");
    write_global_terms(&repo, &fixture);

    let before_igo_section = {
        let content = &fixture;
        let start = content.find("IGO:").unwrap();
        content[start..].to_string()
    };

    Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "learn", "--term", "cafed", "--alias", "卡维地"])
        .output()
        .unwrap();

    let after = fs::read_to_string(repo.path().join("minds-terms.yaml")).unwrap();
    let after_igo_start = after.find("IGO:").unwrap();
    let after_igo_section = &after[after_igo_start..];

    assert_eq!(before_igo_section, after_igo_section, "IGO section should be unchanged");
}
