use assert_cmd::Command;
use tempfile::TempDir;

mod common;

fn setup_with_projects() -> TempDir {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    common::create_project(&repo, "beta");
    // Populate minds.yaml with project entries (matching ProjectEntry Obj variant)
    let manifest = r#"schema_version: '1'
projects_dir: '.'
projects:
  - name: alpha
    path: ./alpha
    created_at: '2026-05-08T10:00:00Z'
  - name: beta
    path: ./beta
    created_at: '2026-05-08T11:00:00Z'
"#;
    std::fs::write(repo.path().join("minds.yaml"), manifest).unwrap();
    repo
}

fn seed_article_in_project(repo: &TempDir, project_name: &str) {
    let yaml = format!(
        "schema_version: '1'\narticles:\n  - title: 'Test Article'\n    project: '{project_name}'\n    article_type: blog\n    article_path: 'docs/test-article.md'\n    status: draft\n    created_at: '2026-05-08T10:00:00Z'\n    updated_at: '2026-05-08T10:00:00Z'\n"
    );
    common::write_index(repo, project_name, &yaml);
    let docs = repo.path().join(project_name).join("docs");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(docs.join("test-article.md"), "# Test Article\ncontent\n").unwrap();
}

// ---------------------------------------------------------------------------
// 1. rename_project_success — project directory renamed, minds.yaml updated
// ---------------------------------------------------------------------------

#[test]
fn rename_project_success() {
    let repo = setup_with_projects();
    seed_article_in_project(&repo, "alpha");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "project", "rename", "alpha", "gamma"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    // Old directory should not exist
    assert!(!repo.path().join("alpha").exists(), "old project directory should be renamed");
    // New directory should exist
    assert!(repo.path().join("gamma").exists(), "new project directory should exist");

    // minds.yaml should be updated
    let manifest = std::fs::read_to_string(repo.path().join("minds.yaml")).unwrap();
    assert!(!manifest.contains("alpha"), "old name should be gone from manifest: {manifest}");
    assert!(manifest.contains("gamma"), "new name should be in manifest: {manifest}");

    // mind-index.yaml project field should be updated
    let index = std::fs::read_to_string(repo.path().join("gamma/mind-index.yaml")).unwrap();
    assert!(
        index.contains("project: 'gamma'") || index.contains("project: gamma"),
        "index project should be updated: {index}"
    );
}

// ---------------------------------------------------------------------------
// 2. rename_project_not_found — unknown project → usage error
// ---------------------------------------------------------------------------

#[test]
fn rename_project_not_found() {
    let repo = setup_with_projects();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "project", "rename", "nonexistent", "whatever"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not found"), "stderr: {stderr}");
}

// ---------------------------------------------------------------------------
// 3. rename_project_json_envelope — JSON output with expected fields
// ---------------------------------------------------------------------------

#[test]
fn rename_project_json_envelope() {
    let repo = setup_with_projects();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "project", "rename", "alpha", "gamma", "--format", "json"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    assert_eq!(v["data"]["old_name"], "alpha");
    assert_eq!(v["data"]["new_name"], "gamma");
    assert!(v["data"]["from"].as_str().is_some_and(|s| s.contains("alpha")));
    assert!(v["data"]["to"].as_str().is_some_and(|s| s.contains("gamma")));
}

// ---------------------------------------------------------------------------
// 4. rename_project_duplicate — renaming to existing project fails
// ---------------------------------------------------------------------------

#[test]
fn rename_project_duplicate() {
    let repo = setup_with_projects();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "project", "rename", "alpha", "beta"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("already exists") || stderr.contains("file_exists") || stderr.contains("refusing to overwrite"),
        "stderr: {stderr}"
    );
}
