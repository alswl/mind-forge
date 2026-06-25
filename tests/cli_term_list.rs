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

fn mf(repo: &common::TempDir) -> Command {
    let mut c = Command::cargo_bin("mf").unwrap();
    c.args(["--root", repo.path().to_str().unwrap()]);
    c
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
        .args(["--root", repo.path().to_str().unwrap(), "--output", "json", "term", "list", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["status"], "ok");
    let terms = parsed["data"]["terms"].as_array().unwrap();
    assert_eq!(terms.len(), 3);
    for item in terms {
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

// ---------------------------------------------------------------------------
// 8. --tag filter (T065)
// ---------------------------------------------------------------------------

#[test]
fn list_filter_by_tag_matches() {
    let (repo, _) = setup();
    let out = mf(&repo).args(["--project", "alpha", "term", "list", "--tag", "infra"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("Mind Repo"), "infra tag should match Mind Repo: {stdout}");
    assert!(!stdout.contains("mf"), "mf has no infra tag: {stdout}");
}

#[test]
fn list_filter_by_tag_no_match() {
    let (repo, _) = setup();
    let out = mf(&repo).args(["--project", "alpha", "term", "list", "--tag", "nonexistent"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("No terms found."), "no match expected: {stdout}");
}

#[test]
fn list_filter_by_alias() {
    let (repo, _) = setup();
    let out = mf(&repo).args(["--project", "alpha", "term", "list", "--alias", "mr"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("Mind Repo"), "alias mr should match Mind Repo: {stdout}");
    assert!(!stdout.contains("mf"), "mf has no alias mr: {stdout}");
}

#[test]
fn list_filter_has_correction() {
    let (repo, _) = setup();
    let out = mf(&repo).args(["--project", "alpha", "term", "list", "--has-correction"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("Mind Repo"), "Mind Repo has corrections: {stdout}");
    assert!(!stdout.contains("mf"), "mf has no corrections: {stdout}");
    assert!(!stdout.contains("alpha"), "alpha has no corrections: {stdout}");
}

#[test]
fn list_filter_combined_tag_and_correction() {
    let (repo, _) = setup();
    // infra tag AND has-correction → only Mind Repo qualifies
    let out =
        mf(&repo).args(["--project", "alpha", "term", "list", "--tag", "infra", "--has-correction"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("Mind Repo"), "Mind Repo matches both filters: {stdout}");
    // mf has cli tag but no correction → excluded
    assert!(!stdout.contains("\tmf\t") && !stdout.contains(" mf "), "mf should be excluded: {stdout}");
}

// ---------------------------------------------------------------------------
// 9. --scope filter (T066)
// ---------------------------------------------------------------------------

#[test]
fn list_scope_project_only() {
    let (repo, _) = setup();
    // Seed a global term that is NOT in the project
    mf(&repo).args(["term", "new", "GlobalOnly", "--definition", "global"]).output().unwrap();

    let out = mf(&repo).args(["--project", "alpha", "term", "list", "--scope", "project"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("Mind Repo"), "project terms should appear: {stdout}");
    assert!(!stdout.contains("GlobalOnly"), "--scope project must not include global: {stdout}");
}

#[test]
fn list_scope_global_only() {
    let (repo, _) = setup();
    mf(&repo).args(["term", "new", "GlobalOnly", "--definition", "global"]).output().unwrap();

    let out = mf(&repo).args(["--project", "alpha", "term", "list", "--scope", "global"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("GlobalOnly"), "global term should appear: {stdout}");
    assert!(!stdout.contains("Mind Repo"), "--scope global must not include project terms: {stdout}");
}

#[test]
fn list_scope_all_merges_both() {
    let (repo, _) = setup();
    mf(&repo).args(["term", "new", "GlobalOnly", "--definition", "global"]).output().unwrap();

    let out = mf(&repo).args(["--project", "alpha", "term", "list", "--scope", "all"]).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("Mind Repo"), "project term should appear: {stdout}");
    assert!(stdout.contains("GlobalOnly"), "global term should appear: {stdout}");
}

// ---------------------------------------------------------------------------
// 10. JSON list filters (T067)
// ---------------------------------------------------------------------------

#[test]
fn list_json_has_correction_filter() {
    let (repo, _) = setup();
    let out = mf(&repo).args(["--project", "alpha", "--json", "term", "list", "--has-correction"]).output().unwrap();
    assert!(out.status.success());
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON");
    let items = json["data"]["terms"].as_array().expect("terms array");
    assert!(
        items.iter().all(|t| t["corrections"].as_array().is_some_and(|c| !c.is_empty())),
        "all returned terms must have at least one correction"
    );
}

#[test]
fn list_json_scope_field_present() {
    let (repo, _) = setup();
    mf(&repo).args(["term", "new", "GlobalOnly"]).output().unwrap();

    let out = mf(&repo).args(["--project", "alpha", "--json", "term", "list", "--scope", "all"]).output().unwrap();
    assert!(out.status.success());
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON");
    let items = json["data"]["terms"].as_array().expect("terms array");
    // Every item that originated from global scope must carry a scope field
    let global_items: Vec<_> = items.iter().filter(|t| t["scope"] == "global").collect();
    assert!(!global_items.is_empty(), "at least one global item expected: {json}");
    let global_only = global_items.iter().find(|t| t["term"] == "GlobalOnly");
    assert!(global_only.is_some(), "GlobalOnly should be present with scope=global");
}

#[test]
fn list_json_deterministic_order() {
    let (repo, _) = setup();
    let out1 = mf(&repo).args(["--project", "alpha", "--json", "term", "list"]).output().unwrap();
    let out2 = mf(&repo).args(["--project", "alpha", "--json", "term", "list"]).output().unwrap();
    assert_eq!(out1.stdout, out2.stdout, "term list JSON must be deterministic");
}
