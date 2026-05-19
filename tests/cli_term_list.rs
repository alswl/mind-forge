use assert_cmd::Command;

mod common;

fn seed_terms(repo: &common::TempDir, project_name: &str) {
    let yaml = r#"schema_version: '1'
terms:
  - term: Mind Repo
    definition: 项目仓库根
    aliases:
      - mr
      - mindrepo
    tags:
      - infra
      - product
    corrections:
      - original: mindrepo
        correct: Mind Repo
  - term: mf
    definition: mind-forge CLI binary
    aliases: []
    tags:
      - cli
    corrections: []
  - term: alpha
    definition: first project
    aliases:
      - a
    tags: []
    corrections: []
"#;
    common::write_index(repo, project_name, yaml);
}

fn setup() -> (common::TempDir, std::path::PathBuf) {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    std::fs::create_dir_all(project.join("docs")).unwrap();
    seed_terms(&repo, "alpha");
    (repo, project)
}

// ---------------------------------------------------------------------------
// 1. alphabetical order
// ---------------------------------------------------------------------------

#[test]
fn list_terms_alpha_sorted() {
    let (repo, _project) = setup();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "list", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    // Order (case-sensitive): Mind Repo (M=77), alpha (a=97), mf (m=109)
    let pos_mr = stdout.find("Mind Repo").unwrap();
    let pos_alpha = stdout.find("alpha").unwrap();
    let pos_mf = stdout.find("\nmf ").or_else(|| stdout.find("\nmf\n")).unwrap();
    assert!(pos_mr < pos_alpha, "Mind Repo (M) before alpha (a) in ASCII sort: {stdout}");
    assert!(pos_alpha < pos_mf, "alpha before mf: {stdout}");
}

// ---------------------------------------------------------------------------
// 2. empty message
// ---------------------------------------------------------------------------

#[test]
fn list_terms_empty_message() {
    let repo = common::setup_repo();
    common::create_project(&repo, "empty");
    std::fs::create_dir_all(repo.path().join("empty/docs")).unwrap();
    common::write_index(&repo, "empty", "schema_version: '1'\n");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "list", "--project", "empty"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("No terms found."));
}

// ---------------------------------------------------------------------------
// 3. JSON shape
// ---------------------------------------------------------------------------

#[test]
fn list_terms_json_shape() {
    let (repo, _project) = setup();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "--format", "json", "term", "list", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["status"], "ok");
    let data = parsed["data"].as_array().unwrap();
    assert_eq!(data.len(), 3);
    for item in data {
        assert!(item.get("term").is_some());
        assert!(item.get("definition").is_some());
        assert!(item.get("aliases").is_some());
        assert!(item.get("tags").is_some());
        assert!(item.get("corrections").is_some());
    }
}

// ---------------------------------------------------------------------------
// 4. --filter substring (case-insensitive)
// ---------------------------------------------------------------------------

#[test]
fn list_terms_filter_substring() {
    let (repo, _project) = setup();
    // "mind" matches Mind Repo's term, aliases, and tags
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "list", "--project", "alpha", "--filter", "mind"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Mind Repo"), "should match Mind Repo: {stdout}");
    assert!(!stdout.contains("mf"), "should not match mf: {stdout}");

    // "CLI" (case-insensitive) should match mf's tag
    let output2 = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "list", "--project", "alpha", "--filter", "CLI"])
        .output()
        .unwrap();

    assert!(output2.status.success());
    let stdout2 = String::from_utf8(output2.stdout).unwrap();
    assert!(stdout2.contains("mf"), "should match mf via tag: {stdout2}");
}

// ---------------------------------------------------------------------------
// 5. --project flag
// ---------------------------------------------------------------------------

#[test]
fn list_terms_with_project_flag() {
    let (repo, _project) = setup();
    // Create another project with no terms
    common::create_project(&repo, "beta");
    std::fs::create_dir_all(repo.path().join("beta/docs")).unwrap();
    common::write_index(&repo, "beta", "schema_version: '1'\n");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "list", "--project", "beta"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("No terms found."));
}

// ---------------------------------------------------------------------------
// 6. outside mind repo
// ---------------------------------------------------------------------------

#[test]
fn list_terms_outside_repo() {
    let output = Command::cargo_bin("mf").unwrap().args(["term", "list"]).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not in a mind repo"));
}

// ---------------------------------------------------------------------------
// ── US1: repo format global terms ─────────────────────────────────────────

fn write_global_terms(repo: &common::TempDir, content: &str) {
    std::fs::write(repo.path().join("minds-terms.yaml"), content).unwrap();
}

fn repo_format_fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/term_repo_format").join(name),
    )
    .unwrap()
}

// T015
#[test]
fn list_repo_format_text() {
    let repo = common::setup_repo();
    write_global_terms(&repo, &repo_format_fixture("simple.yaml"));

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "list"])
        .output()
        .unwrap();

    assert!(output.status.success(), "exit 0 for repo-format file");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("cafed"), "should list cafed: {stdout}");
    assert!(stdout.contains("IGO"), "should list IGO: {stdout}");
    assert!(stdout.contains("卿祤"), "should list 卿祤: {stdout}");
    // CORRECTIONS column shows count (2 corrections per entry)
    assert!(stdout.contains("2"), "should show correction count: {stdout}");
}

// T016
#[test]
fn list_repo_format_json() {
    let repo = common::setup_repo();
    write_global_terms(&repo, &repo_format_fixture("simple.yaml"));

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "--format", "json", "term", "list"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["status"], "ok");
    let data = parsed["data"].as_array().unwrap();
    assert_eq!(data.len(), 3, "3 terms from simple.yaml");
    for item in data.as_slice() {
        assert_eq!(item["definition"], serde_json::Value::Null);
        assert_eq!(item["aliases"].as_array().unwrap().len(), 0);
        assert_eq!(item["tags"].as_array().unwrap().len(), 0);
        let corr = item["corrections"].as_array().unwrap();
        for c in corr {
            assert_eq!(c["correct"], item["term"]);
        }
    }
}

// T017
#[test]
fn list_repo_format_with_comments() {
    let repo = common::setup_repo();
    write_global_terms(&repo, &repo_format_fixture("simple.yaml"));

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "list"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.is_empty(), "no stderr noise: {stderr}");
}

// T018
#[test]
fn list_rejects_duplicate_top_level_key() {
    let repo = common::setup_repo();
    write_global_terms(&repo, &repo_format_fixture("duplicate.yaml"));

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "list"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1), "exit 1 for duplicate key");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("duplicate term"), "stderr should mention duplicate term: {stderr}");
}

// T019
#[test]
fn list_rejects_malformed_file() {
    let repo = common::setup_repo();
    write_global_terms(&repo, &repo_format_fixture("malformed.yaml"));

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "list"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1), "exit 1 for malformed file");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("unsupported file shape"), "should name both supported shapes: {stderr}");
}

// 7. missing index → empty list, exit 0
// ---------------------------------------------------------------------------

#[test]
fn list_terms_index_missing_or_empty() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    // No index file at all
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "list", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("No terms found."));

    // Empty terms list should also show empty
    common::write_index(&repo, "alpha", "schema_version: '1'\nterms: []\n");
    let output2 = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "term", "list", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output2.status.success());
    let stdout2 = String::from_utf8(output2.stdout).unwrap();
    assert!(stdout2.contains("No terms found."));
}
