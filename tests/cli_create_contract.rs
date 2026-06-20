use assert_cmd::Command;
use std::fs;

mod common;

// ── helpers ──────────────────────────────────────────────────────────────────

fn mf(repo: &common::TempDir) -> Command {
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.args(["--root", repo.path().to_str().unwrap()]);
    cmd
}

fn mf_json(repo: &common::TempDir) -> Command {
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.args(["--root", repo.path().to_str().unwrap(), "--format", "json"]);
    cmd
}

fn setup_project() -> (common::TempDir, std::path::PathBuf) {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    fs::create_dir_all(project.join("docs")).unwrap();
    common::write_index(&repo, "alpha", "schema_version: '1'\n");
    (repo, project)
}

// ── project new ──────────────────────────────────────────────────────────────

#[test]
fn project_new_text_confirm() {
    let repo = common::setup_repo();
    let output = mf(&repo).args(["project", "new", "demo"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("✓ created project: demo"), "stdout: {stdout}");
}

#[test]
fn project_new_json_envelope() {
    let repo = common::setup_repo();
    let output = mf_json(&repo).args(["project", "new", "demo"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    let data = &v["data"];
    assert!(data.is_object(), "data must be object");
    assert_eq!(data["kind"], "project");
    assert_eq!(data["identity"], "demo");
    assert_eq!(data["dry_run"], false);
    assert!(data["path"].as_str().is_some());
}

#[test]
fn project_new_dry_run_no_mutation() {
    let repo = common::setup_repo();
    let output = mf(&repo).args(["project", "new", "demo", "--dry-run"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("[dry-run] would create project: demo"), "stdout: {stdout}");
    // No filesystem mutation
    assert!(!repo.path().join("demo").exists());
}

#[test]
fn project_new_dry_run_json() {
    let repo = common::setup_repo();
    let output = mf_json(&repo).args(["project", "new", "demo", "--dry-run"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["data"]["dry_run"], true);
    assert_eq!(v["data"]["kind"], "project");
}

// ── article new ──────────────────────────────────────────────────────────────

#[test]
fn article_new_text_confirm() {
    let (repo, _project) = setup_project();
    let output = mf(&repo).args(["article", "new", "my-article", "--project", "alpha"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("✓ created article:"), "stdout: {stdout}");
}

#[test]
fn article_new_json_envelope() {
    let (repo, _project) = setup_project();
    let output = mf_json(&repo).args(["article", "new", "my-article", "--project", "alpha"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    let data = &v["data"];
    assert!(data.is_object());
    assert_eq!(data["kind"], "article");
    assert!(data["identity"].as_str().is_some());
    assert_eq!(data["dry_run"], false);
    assert!(data["details"].is_object());
}

#[test]
fn article_new_dry_run_no_mutation() {
    let (repo, project) = setup_project();
    let output = mf(&repo).args(["article", "new", "my-article", "--project", "alpha", "--dry-run"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("[dry-run] would create article:"), "stdout: {stdout}");
    // No filesystem mutation
    let docs = project.join("docs");
    let entries = fs::read_dir(&docs).unwrap().count();
    assert_eq!(entries, 0, "no files should be created in docs during dry-run");
}

// ── term new ─────────────────────────────────────────────────────────────────

#[test]
fn term_new_text_confirm() {
    let repo = common::setup_repo();
    // Project-scoped term
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    fs::create_dir_all(project.join("docs")).unwrap();
    common::write_index(&repo, "alpha", "schema_version: '1'\n");

    let output = mf(&repo)
        .args(["term", "new", "RAG", "--definition", "Retrieval-Augmented Generation", "--project", "alpha"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("created term \"RAG\""), "stdout: {stdout}");
}

#[test]
fn term_new_json_envelope() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    fs::create_dir_all(project.join("docs")).unwrap();
    common::write_index(&repo, "alpha", "schema_version: '1'\n");

    let output =
        mf_json(&repo).args(["term", "new", "RAG", "--definition", "desc", "--project", "alpha"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    let data = &v["data"];
    assert!(data.is_object());
    assert_eq!(data["term"], "RAG");
    assert_eq!(data["created"], true);
    assert!(data["added_aliases"].is_array());
    assert!(data["added_tags"].is_array());
}

#[test]
fn term_new_dry_run_no_mutation() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    fs::create_dir_all(project.join("docs")).unwrap();
    common::write_index(&repo, "alpha", "schema_version: '1'\n");

    let initial_index = fs::read_to_string(project.join("mind-index.yaml")).unwrap();

    let output = mf(&repo)
        .args(["term", "new", "RAG", "--definition", "desc", "--project", "alpha", "--dry-run"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("[dry-run] would create term: RAG"), "stdout: {stdout}");

    // No mutation
    let after_index = fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert_eq!(initial_index, after_index, "index should not change in dry-run");
}

// ── source add ───────────────────────────────────────────────────────────────

#[test]
fn source_add_text_confirm() {
    let (repo, _project) = setup_project();
    // Create a temp source file
    let source_dir = tempfile::TempDir::new().unwrap();
    let source_file = source_dir.path().join("paper.pdf");
    fs::write(&source_file, b"fake pdf content").unwrap();

    let output =
        mf(&repo).args(["source", "new", source_file.to_str().unwrap(), "--project", "alpha"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("✓ added source:"), "stdout: {stdout}");
}

#[test]
fn source_add_json_envelope() {
    let (repo, _project) = setup_project();
    let source_dir = tempfile::TempDir::new().unwrap();
    let source_file = source_dir.path().join("paper.pdf");
    fs::write(&source_file, b"fake pdf content").unwrap();

    let output =
        mf_json(&repo).args(["source", "new", source_file.to_str().unwrap(), "--project", "alpha"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    let data = &v["data"];
    assert!(data.is_object());
    assert_eq!(data["kind"], "source");
    assert!(data["identity"].as_str().is_some());
    assert_eq!(data["dry_run"], false);
}

#[test]
fn source_add_dry_run_no_mutation() {
    let (repo, project) = setup_project();
    let source_dir = tempfile::TempDir::new().unwrap();
    let source_file = source_dir.path().join("paper.pdf");
    fs::write(&source_file, b"fake pdf content").unwrap();

    let output = mf(&repo)
        .args(["source", "new", source_file.to_str().unwrap(), "--project", "alpha", "--dry-run"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("[dry-run] would add source:"), "stdout: {stdout}");

    // No filesystem mutation
    let sources_dir = project.join("sources");
    if sources_dir.exists() {
        assert!(fs::read_dir(&sources_dir).unwrap().count() == 0, "no sources should be added during dry-run");
    }
}

// ── asset add ────────────────────────────────────────────────────────────────

#[test]
fn asset_add_text_confirm() {
    let (repo, project) = setup_project();
    fs::create_dir_all(project.join("assets")).unwrap();
    let asset_file = common::TempDir::new().unwrap();
    let asset_path = asset_file.path().join("chart.png");
    fs::write(&asset_path, b"fake png").unwrap();

    let output = mf(&repo).args(["asset", "new", asset_path.to_str().unwrap(), "--project", "alpha"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("✓ added asset:"), "stdout: {stdout}");
}

#[test]
fn asset_add_json_envelope() {
    let (repo, project) = setup_project();
    fs::create_dir_all(project.join("assets")).unwrap();
    let asset_file = common::TempDir::new().unwrap();
    let asset_path = asset_file.path().join("chart.png");
    fs::write(&asset_path, b"fake png").unwrap();

    let output =
        mf_json(&repo).args(["asset", "new", asset_path.to_str().unwrap(), "--project", "alpha"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    let data = &v["data"];
    assert!(data.is_object());
    assert_eq!(data["kind"], "asset");
    assert!(data["identity"].as_str().is_some());
    assert_eq!(data["dry_run"], false);
}

#[test]
fn asset_add_dry_run_no_mutation() {
    let (repo, project) = setup_project();
    fs::create_dir_all(project.join("assets")).unwrap();
    let asset_file = common::TempDir::new().unwrap();
    let asset_path = asset_file.path().join("chart.png");
    fs::write(&asset_path, b"fake png").unwrap();

    let output = mf(&repo)
        .args(["asset", "new", asset_path.to_str().unwrap(), "--project", "alpha", "--dry-run"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("[dry-run] would add asset:"), "stdout: {stdout}");

    // No filesystem mutation
    let assets_dir = project.join("assets");
    if assets_dir.exists() {
        assert!(fs::read_dir(&assets_dir).unwrap().count() == 0, "no assets should be added during dry-run");
    }
}

// ── term add / learn (term correction) ───────────────────────────────────────

#[test]
fn term_add_text_confirm() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    fs::create_dir_all(project.join("docs")).unwrap();
    let index_yaml = r#"schema_version: '1'
terms:
  - term: Mind Repo
    definition: desc
    aliases: []
    tags: []
    corrections: []
"#;
    common::write_index(&repo, "alpha", index_yaml);

    let output =
        mf(&repo).args(["term", "new", "Mind Repo", "--alias", "mindrepo", "--project", "alpha"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("added alias") || stdout.contains("created term"), "stdout: {stdout}");
}

#[test]
fn term_add_json_envelope() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    fs::create_dir_all(project.join("docs")).unwrap();
    let index_yaml = r#"schema_version: '1'
terms:
  - term: Mind Repo
    definition: desc
    aliases: []
    tags: []
    corrections: []
"#;
    common::write_index(&repo, "alpha", index_yaml);

    let output = mf_json(&repo)
        .args(["term", "new", "Mind Repo", "--alias", "mindrepo", "--project", "alpha"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    let data = &v["data"];
    assert!(data.is_object());
    assert_eq!(data["term"], "Mind Repo");
    assert_eq!(data["created"], false);
    assert!(!data["added_aliases"].as_array().unwrap().is_empty());
}

#[test]
fn term_add_dry_run_no_mutation() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    fs::create_dir_all(project.join("docs")).unwrap();
    let index_yaml = r#"schema_version: '1'
terms:
  - term: Mind Repo
    definition: desc
    aliases: []
    tags: []
    corrections: []
"#;
    common::write_index(&repo, "alpha", index_yaml);
    let initial = fs::read_to_string(project.join("mind-index.yaml")).unwrap();

    let output = mf(&repo)
        .args(["term", "new", "Mind Repo", "--alias", "mindrepo", "--project", "alpha", "--dry-run"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("[dry-run] would create term:"), "stdout: {stdout}");

    // No mutation
    let after = fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert_eq!(initial, after, "index should not change in dry-run");
}

// ── common json envelope shape test ──────────────────────────────────────────

#[test]
fn all_create_json_data_is_object() {
    // Verifies FR-070: data is always an object for all create/add commands
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    fs::create_dir_all(project.join("docs")).unwrap();
    common::write_index(&repo, "alpha", "schema_version: '1'\n");

    // project new
    let out = mf_json(&repo).args(["project", "new", "demo"]).output().unwrap();
    let v: serde_json::Value = serde_json::from_str(&String::from_utf8(out.stdout).unwrap()).unwrap();
    assert!(v["data"].is_object(), "project new: data should be object");

    // article new
    let out = mf_json(&repo).args(["article", "new", "art", "--project", "alpha"]).output().unwrap();
    let v: serde_json::Value = serde_json::from_str(&String::from_utf8(out.stdout).unwrap()).unwrap();
    assert!(v["data"].is_object(), "article new: data should be object");

    // term new (global)
    let out = mf_json(&repo).args(["term", "new", "Test"]).output().unwrap();
    let v: serde_json::Value = serde_json::from_str(&String::from_utf8(out.stdout).unwrap()).unwrap();
    assert!(v["data"].is_object(), "term new: data should be object");

    // source add
    let sf = tempfile::TempDir::new().unwrap();
    let s = sf.path().join("f.pdf");
    fs::write(&s, b"x").unwrap();
    let out = mf_json(&repo).args(["source", "new", s.to_str().unwrap(), "--project", "alpha"]).output().unwrap();
    let v: serde_json::Value = serde_json::from_str(&String::from_utf8(out.stdout).unwrap()).unwrap();
    assert!(v["data"].is_object(), "source add: data should be object");

    // asset add
    fs::create_dir_all(project.join("assets")).unwrap();
    let af = tempfile::TempDir::new().unwrap();
    let a = af.path().join("c.png");
    fs::write(&a, b"x").unwrap();
    let out = mf_json(&repo).args(["asset", "new", a.to_str().unwrap(), "--project", "alpha"]).output().unwrap();
    let v: serde_json::Value = serde_json::from_str(&String::from_utf8(out.stdout).unwrap()).unwrap();
    assert!(v["data"].is_object(), "asset add: data should be object");

    // term add needs an existing term
    let index_yaml = r#"schema_version: '1'
terms:
  - term: Mind Repo
    definition: desc
    aliases: []
    tags: []
    corrections: []
"#;
    common::write_index(&repo, "alpha", index_yaml);
    let out =
        mf_json(&repo).args(["term", "new", "Mind Repo", "--alias", "mr", "--project", "alpha"]).output().unwrap();
    let v: serde_json::Value = serde_json::from_str(&String::from_utf8(out.stdout).unwrap()).unwrap();
    assert!(v["data"].is_object(), "term add: data should be object");
}
