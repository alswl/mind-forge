use assert_cmd::Command;
use tempfile::TempDir;

mod common;

fn seed_sources(repo: &TempDir, project_name: &str) {
    let yaml = r#"schema_version: '1'
sources:
  - name: paper
    type: pdf
    path: sources/pdf/paper.pdf
    tags: []
    added_at: '2026-05-08T10:00:00Z'
    updated_at: '2026-05-08T10:00:00Z'
  - name: research-blog
    type: web
    url: https://example.com/research
    path: ~
    tags: []
    added_at: '2026-05-08T11:00:00Z'
    updated_at: '2026-05-08T11:00:00Z'
  - name: reading-notes
    type: file
    path: sources/file/notes.md
    tags: []
    added_at: '2026-05-08T12:00:00Z'
    updated_at: '2026-05-08T12:00:00Z'
  - name: example-feed
    type: rss
    url: https://example.com/feed.xml
    path: ~
    tags: []
    added_at: '2026-05-08T13:00:00Z'
    updated_at: '2026-05-08T13:00:00Z'
"#;
    // Replace ~ with null for valid YAML
    let yaml = yaml.replace("path: ~", "path:");
    common::write_index(repo, project_name, &yaml);
}

fn setup() -> (TempDir, std::path::PathBuf) {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    std::fs::create_dir_all(project.join("sources")).unwrap();
    seed_sources(&repo, "alpha");
    (repo, project)
}

// ---------------------------------------------------------------------------
// 1. list_table_format — text table ≥3 columns, alphabetical
// ---------------------------------------------------------------------------

#[test]
fn list_table_format() {
    let (repo, _project) = setup();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "list", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();

    // alphabetical: example-feed < paper < reading-notes < research-blog
    let feed_pos = stdout.find("example-feed").unwrap();
    let paper_pos = stdout.find("paper").unwrap();
    assert!(feed_pos < paper_pos, "should be alphabetical");
    // Data columns must be present
    assert!(stdout.contains("example-feed"));
    assert!(stdout.contains("paper"));
}

// ---------------------------------------------------------------------------
// 2. list_empty_friendly_message — empty index → "No sources found."
// ---------------------------------------------------------------------------

#[test]
fn list_empty_friendly_message() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "list", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("No sources found."));
}

// ---------------------------------------------------------------------------
// 3. list_json_envelope — --format json → data array
// ---------------------------------------------------------------------------

#[test]
fn list_json_envelope() {
    let (repo, _project) = setup();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "list", "--project", "alpha", "--format", "json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    assert!(v["data"]["sources"].is_array());
    assert_eq!(v["data"]["sources"].as_array().unwrap().len(), 4);
}

// ---------------------------------------------------------------------------
// 4. list_filter_by_name_substring — --filter matches name case-insensitively
// ---------------------------------------------------------------------------

#[test]
fn list_filter_by_name_substring() {
    let (repo, _project) = setup();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "list", "--project", "alpha", "--filter", "PAPER"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("paper"));
    assert!(!stdout.contains("reading-notes"));
}

// ---------------------------------------------------------------------------
// 5. list_type_filter -- --type rss only
// ---------------------------------------------------------------------------

#[test]
fn list_type_filter() {
    let (repo, _project) = setup();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "list", "--project", "alpha", "--type", "rss"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("example-feed"));
    assert!(!stdout.contains("paper"));
}

// ---------------------------------------------------------------------------
// 6. list_invalid_type_value -- --type unknown → clap exit 2
// ---------------------------------------------------------------------------

#[test]
fn list_invalid_type_value() {
    let (repo, _project) = setup();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "list", "--project", "alpha", "--type", "unknown"])
        .output()
        .unwrap();

    assert_eq!(output.status.code().unwrap_or_default(), 2);
}

// ---------------------------------------------------------------------------
// 7. list_project_flag — --project switches context
// ---------------------------------------------------------------------------

#[test]
fn list_project_flag() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    common::create_project(&repo, "beta");
    // Seed only beta
    std::fs::create_dir_all(repo.path().join("beta/sources")).unwrap();
    let yaml = r#"schema_version: '1'
sources:
  - name: beta-only
    type: pdf
    path: sources/pdf/beta.pdf
    tags: []
    added_at: '2026-05-08T10:00:00Z'
    updated_at: '2026-05-08T10:00:00Z'
"#;
    common::write_index(&repo, "beta", yaml);

    // List alpha → empty
    let output_alpha = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "list", "--project", "alpha"])
        .output()
        .unwrap();
    assert!(output_alpha.status.success());
    assert!(String::from_utf8(output_alpha.stdout).unwrap().contains("No sources found."));

    // List beta → has entry
    let output_beta = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "list", "--project", "beta"])
        .output()
        .unwrap();
    assert!(output_beta.status.success());
    assert!(String::from_utf8(output_beta.stdout).unwrap().contains("beta-only"));
}

// ---------------------------------------------------------------------------
// 8. list_outside_repo — repo not found → error
// ---------------------------------------------------------------------------

#[test]
fn list_outside_repo() {
    let outside = TempDir::new().unwrap();
    let output =
        Command::cargo_bin("mf").unwrap().args(["source", "list"]).current_dir(outside.path()).output().unwrap();

    assert!(!output.status.success());
}

// ---------------------------------------------------------------------------
// 9. list_filter_and_type_intersection — both present → AND
// ---------------------------------------------------------------------------

#[test]
fn list_filter_and_type_intersection() {
    let (repo, _project) = setup();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "source",
            "list",
            "--project",
            "alpha",
            "--filter",
            "paper",
            "--type",
            "pdf",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    // Should only match "paper" which is both "paper" in name AND pdf
    assert!(stdout.contains("paper"));
    assert!(!stdout.contains("reading-notes"));
    assert!(!stdout.contains("research-blog"));
}
