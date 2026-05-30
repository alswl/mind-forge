use assert_cmd::Command;
use std::fs;

mod common;

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

fn setup_project_with_docs(name: &str) -> (common::TempDir, std::path::PathBuf) {
    let repo = common::setup_repo();
    common::create_project(&repo, name);
    let project = repo.path().join(name);
    fs::create_dir_all(project.join("docs")).unwrap();
    fs::create_dir_all(project.join("sources")).unwrap();
    fs::create_dir_all(project.join("assets")).unwrap();
    let index_yaml = r#"schema_version: '1'
terms:
  - term: TestTerm
    definition: A test term
    aliases: []
    tags: []
    corrections: []
"#;
    common::write_index(&repo, name, index_yaml);
    (repo, project)
}

fn setup_project_with_source(name: &str, source_name: &str) -> (common::TempDir, std::path::PathBuf) {
    let (repo, project) = setup_project_with_docs(name);
    let source_dir = tempfile::TempDir::new().unwrap();
    let source_file = source_dir.path().join(format!("{}.pdf", source_name));
    fs::write(&source_file, b"fake pdf content").unwrap();

    // Add the source first so we can rename/remove it
    let _ = mf(&repo)
        .args(["source", "add", source_file.to_str().unwrap(), "--name", source_name, "--project", name])
        .output()
        .unwrap();
    (repo, project)
}

fn setup_project_with_asset(name: &str, asset_name: &str) -> (common::TempDir, std::path::PathBuf) {
    let (repo, project) = setup_project_with_docs(name);
    let asset_file = common::TempDir::new().unwrap();
    let asset_path = asset_file.path().join(asset_name);
    fs::write(&asset_path, b"fake asset").unwrap();

    // Add the asset first
    let _ = mf(&repo)
        .args(["asset", "add", asset_path.to_str().unwrap(), "--name", asset_name, "--project", name])
        .output()
        .unwrap();
    (repo, project)
}

// ═══════════════════════════════════════════════════════════════════════════════
// T061: Per-verb envelopes and text confirmation lines
// ═══════════════════════════════════════════════════════════════════════════════

// ── rename ────────────────────────────────────────────────────────────────────

#[test]
fn project_rename_text_confirm() {
    let repo = common::setup_repo();
    common::create_project(&repo, "oldproj");
    let output = mf(&repo).args(["project", "rename", "oldproj", "newproj"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("✓ renamed project: oldproj → newproj"), "stdout: {stdout}");
}

#[test]
fn project_rename_json_envelope() {
    let repo = common::setup_repo();
    common::create_project(&repo, "oldproj");
    let output = mf_json(&repo).args(["project", "rename", "oldproj", "newproj"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    let data = &v["data"];
    assert!(data.is_object());
    assert_eq!(data["kind"], "project");
    assert_eq!(data["new_identity"], "newproj");
    assert_eq!(data["old_identity"], "oldproj");
    assert_eq!(data["dry_run"], false);
}

#[test]
fn project_rename_dry_run() {
    let repo = common::setup_repo();
    common::create_project(&repo, "oldproj");
    let output = mf(&repo).args(["project", "rename", "oldproj", "newproj", "--dry-run"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("[dry-run] would rename project: oldproj → newproj"), "stdout: {stdout}");
    assert!(repo.path().join("oldproj").exists());
    assert!(!repo.path().join("newproj").exists());
}

#[test]
fn article_rename_text_confirm() {
    let (repo, project) = setup_project_with_docs("alpha");
    fs::write(project.join("docs/old-title.md"), b"# Old Title\n\ncontent\n").unwrap();
    let index = r#"schema_version: '1'
articles:
  - title: 'old-title'
    project: 'alpha'
    article_type: blog
    article_path: 'docs/old-title.md'
    status: draft
    created_at: '2026-05-07T00:00:00Z'
    updated_at: '2026-05-07T00:00:00Z'
"#;
    common::write_index(&repo, "alpha", index);

    let output = mf(&repo)
        .args(["article", "rename", "docs/old-title.md", "docs/new-title.md", "--project", "alpha"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("✓ renamed article:"), "stdout: {stdout}");
    assert!(stdout.contains("→"), "stdout should contain arrow: {stdout}");
}

#[test]
fn article_rename_json_envelope() {
    let (repo, project) = setup_project_with_docs("alpha");
    fs::write(project.join("docs/old-title.md"), b"# Old Title\n\ncontent\n").unwrap();
    let index = r#"schema_version: '1'
articles:
  - title: 'old-title'
    project: 'alpha'
    article_type: blog
    article_path: 'docs/old-title.md'
    status: draft
    created_at: '2026-05-07T00:00:00Z'
    updated_at: '2026-05-07T00:00:00Z'
"#;
    common::write_index(&repo, "alpha", index);

    let output = mf_json(&repo)
        .args(["article", "rename", "docs/old-title.md", "docs/new-title.md", "--project", "alpha"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    let data = &v["data"];
    assert!(data.is_object());
    assert_eq!(data["kind"], "article");
    assert!(data["new_identity"].as_str().is_some());
    assert!(data["old_identity"].as_str().is_some());
}

#[test]
fn source_rename_text_confirm() {
    let (repo, _project) = setup_project_with_source("alpha", "old-src");
    let output = mf(&repo).args(["source", "rename", "old-src", "new-src", "--project", "alpha"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("✓ renamed source:"), "stdout: {stdout}");
}

#[test]
fn source_rename_json_envelope() {
    let (repo, _project) = setup_project_with_source("alpha", "old-src");
    let output =
        mf_json(&repo).args(["source", "rename", "old-src", "new-src", "--project", "alpha"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    let data = &v["data"];
    assert!(data.is_object());
    assert_eq!(data["kind"], "source");
    assert_eq!(data["new_identity"], "new-src");
    assert_eq!(data["old_identity"], "old-src");
}

#[test]
fn asset_rename_text_confirm() {
    let (repo, _project) = setup_project_with_asset("alpha", "old-chart.png");
    let output =
        mf(&repo).args(["asset", "rename", "old-chart.png", "new-chart.png", "--project", "alpha"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("✓ renamed asset"), "stdout: {stdout}");
}

#[test]
fn asset_rename_json_envelope() {
    let (repo, _project) = setup_project_with_asset("alpha", "old-chart.png");
    let output = mf_json(&repo)
        .args(["asset", "rename", "old-chart.png", "new-chart.png", "--project", "alpha"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    let data = &v["data"];
    assert!(data.is_object());
    assert_eq!(data["kind"], "asset");
    assert_eq!(data["new_identity"], "new-chart.png");
    assert_eq!(data["old_identity"], "old-chart.png");
}

#[test]
fn term_rename_text_confirm() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let index_yaml = r#"schema_version: '1'
terms:
  - term: OldTerm
    definition: Old definition
    aliases: []
    tags: []
    corrections: []
"#;
    common::write_index(&repo, "alpha", index_yaml);

    let output = mf(&repo).args(["term", "rename", "OldTerm", "NewTerm", "--project", "alpha"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("✓ renamed term: OldTerm → NewTerm"), "stdout: {stdout}");
}

#[test]
fn term_rename_json_envelope() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let index_yaml = r#"schema_version: '1'
terms:
  - term: OldTerm
    definition: Old definition
    aliases: []
    tags: []
    corrections: []
"#;
    common::write_index(&repo, "alpha", index_yaml);

    let output = mf_json(&repo).args(["term", "rename", "OldTerm", "NewTerm", "--project", "alpha"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    let data = &v["data"];
    assert!(data.is_object());
    assert_eq!(data["kind"], "term");
    assert_eq!(data["new_identity"], "NewTerm");
    assert_eq!(data["old_identity"], "OldTerm");
}

// ── remove ────────────────────────────────────────────────────────────────────

#[test]
fn project_remove_text_confirm() {
    let repo = common::setup_repo();
    common::create_project(&repo, "toremove");
    let output = mf(&repo).args(["project", "remove", "toremove", "--yes"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("✓ removed project: toremove"), "stdout: {stdout}");
}

#[test]
fn project_remove_json_envelope() {
    let repo = common::setup_repo();
    common::create_project(&repo, "toremove");
    let output = mf_json(&repo).args(["project", "remove", "toremove", "--yes"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    let data = &v["data"];
    assert!(data.is_object());
    assert_eq!(data["kind"], "project");
    assert_eq!(data["identity"], "toremove");
    assert_eq!(data["removed"], true);
}

#[test]
fn project_remove_dry_run() {
    let repo = common::setup_repo();
    common::create_project(&repo, "toremove");
    let output = mf(&repo).args(["project", "remove", "toremove", "--dry-run", "--yes"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("[dry-run] would remove project: toremove"), "stdout: {stdout}");
    assert!(repo.path().join("toremove").exists());
}

#[test]
fn article_remove_text_confirm() {
    let (repo, project) = setup_project_with_docs("alpha");
    fs::write(project.join("docs/toremove.md"), b"# test\n").unwrap();
    let index = r#"schema_version: '1'
articles:
  - title: 'toremove'
    project: 'alpha'
    article_type: blog
    article_path: 'docs/toremove.md'
    status: draft
    created_at: '2026-05-07T00:00:00Z'
    updated_at: '2026-05-07T00:00:00Z'
"#;
    common::write_index(&repo, "alpha", index);

    let output =
        mf(&repo).args(["article", "remove", "docs/toremove.md", "--project", "alpha", "--yes"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("✓ removed article:"), "stdout: {stdout}");
}

#[test]
fn article_remove_json_envelope() {
    let (repo, project) = setup_project_with_docs("alpha");
    fs::write(project.join("docs/toremove.md"), b"# test\n").unwrap();
    let index = r#"schema_version: '1'
articles:
  - title: 'toremove'
    project: 'alpha'
    article_type: blog
    article_path: 'docs/toremove.md'
    status: draft
    created_at: '2026-05-07T00:00:00Z'
    updated_at: '2026-05-07T00:00:00Z'
"#;
    common::write_index(&repo, "alpha", index);

    let output =
        mf_json(&repo).args(["article", "remove", "docs/toremove.md", "--project", "alpha", "--yes"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    let data = &v["data"];
    assert!(data.is_object());
    assert_eq!(data["kind"], "article");
}

#[test]
fn source_remove_text_confirm() {
    let (repo, _project) = setup_project_with_source("alpha", "toremove-src");
    let output = mf(&repo).args(["source", "remove", "toremove-src", "--project", "alpha", "--yes"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("✓ removed source:"), "stdout: {stdout}");
}

#[test]
fn source_remove_json_envelope() {
    let (repo, _project) = setup_project_with_source("alpha", "toremove-src");
    let output =
        mf_json(&repo).args(["source", "remove", "toremove-src", "--project", "alpha", "--yes"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    let data = &v["data"];
    assert!(data.is_object());
    assert_eq!(data["kind"], "source");
}

#[test]
fn asset_remove_text_confirm() {
    let (repo, _project) = setup_project_with_asset("alpha", "toremove-asset.png");
    let output =
        mf(&repo).args(["asset", "remove", "toremove-asset.png", "--project", "alpha", "--yes"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("✓ removed asset:"), "stdout: {stdout}");
}

#[test]
fn asset_remove_json_envelope() {
    let (repo, _project) = setup_project_with_asset("alpha", "toremove-asset.png");
    let output =
        mf_json(&repo).args(["asset", "remove", "toremove-asset.png", "--project", "alpha", "--yes"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    let data = &v["data"];
    assert!(data.is_object());
    assert_eq!(data["kind"], "asset");
}

#[test]
fn term_remove_text_confirm() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let index_yaml = r#"schema_version: '1'
terms:
  - term: ToRemoveTerm
    definition: To be removed
    aliases: []
    tags: []
    corrections: []
"#;
    common::write_index(&repo, "alpha", index_yaml);

    let output = mf(&repo).args(["term", "remove", "ToRemoveTerm", "--project", "alpha", "--yes"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("✓ removed term: ToRemoveTerm"), "stdout: {stdout}");
}

#[test]
fn term_remove_json_envelope() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let index_yaml = r#"schema_version: '1'
terms:
  - term: ToRemoveTerm
    definition: To be removed
    aliases: []
    tags: []
    corrections: []
"#;
    common::write_index(&repo, "alpha", index_yaml);

    let output =
        mf_json(&repo).args(["term", "remove", "ToRemoveTerm", "--project", "alpha", "--yes"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    let data = &v["data"];
    assert!(data.is_object());
    assert_eq!(data["kind"], "term");
    assert_eq!(data["identity"], "ToRemoveTerm");
    assert_eq!(data["removed"], true);
}

// ── update ────────────────────────────────────────────────────────────────────

#[test]
fn source_update_text_confirm() {
    let (repo, _project) = setup_project_with_source("alpha", "update-src");
    let output = mf(&repo)
        .args(["source", "update", "update-src", "--rename", "renamed-src", "--project", "alpha"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("✓ updated source:"), "stdout: {stdout}");
}

#[test]
fn source_update_json_envelope() {
    let (repo, _project) = setup_project_with_source("alpha", "update-src");
    let output = mf_json(&repo)
        .args(["source", "update", "update-src", "--rename", "renamed-src", "--project", "alpha"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    let data = &v["data"];
    assert!(data.is_object());
    assert_eq!(data["kind"], "source");
}

#[test]
fn term_update_text_confirm() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let index_yaml = r#"schema_version: '1'
terms:
  - term: UpdateTerm
    definition: Old def
    aliases: []
    tags: []
    corrections: []
"#;
    common::write_index(&repo, "alpha", index_yaml);

    let output = mf(&repo)
        .args(["term", "update", "UpdateTerm", "--definition", "New definition", "--project", "alpha"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("✓ updated term:"), "stdout: {stdout}");
}

#[test]
fn term_update_json_envelope() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let index_yaml = r#"schema_version: '1'
terms:
  - term: UpdateTerm
    definition: Old def
    aliases: []
    tags: []
    corrections: []
"#;
    common::write_index(&repo, "alpha", index_yaml);

    let output = mf_json(&repo)
        .args(["term", "update", "UpdateTerm", "--definition", "New definition", "--project", "alpha"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    let data = &v["data"];
    assert!(data.is_object());
    assert_eq!(data["kind"], "term");
}

// ── index ─────────────────────────────────────────────────────────────────────

#[test]
fn article_index_text() {
    let (repo, project) = setup_project_with_docs("alpha");
    fs::write(project.join("docs/test.md"), b"# test\n").unwrap();
    let output = mf(&repo).args(["article", "index", "--project", "alpha"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("indexed article:"), "stdout: {stdout}");
}

#[test]
fn article_index_json() {
    let (repo, project) = setup_project_with_docs("alpha");
    fs::write(project.join("docs/test.md"), b"# test\n").unwrap();
    let output = mf_json(&repo).args(["article", "index", "--project", "alpha"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    let data = &v["data"];
    assert!(data.is_object());
    assert_eq!(data["kind"], "article");
    assert!(data.get("added").is_some());
    assert!(data.get("removed").is_some());
    assert!(data.get("kept_count").is_some());
    assert!(data.get("scanned_count").is_some());
    assert!(data.get("dry_run").is_some());
}

#[test]
fn source_index_text() {
    let (repo, _project) = setup_project_with_source("alpha", "idx-src");
    let output = mf(&repo).args(["source", "index", "--project", "alpha"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("indexed source:"), "stdout: {stdout}");
}

#[test]
fn source_index_json() {
    let (repo, _project) = setup_project_with_source("alpha", "idx-src");
    let output = mf_json(&repo).args(["source", "index", "--project", "alpha"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    let data = &v["data"];
    assert!(data.is_object());
    assert_eq!(data["kind"], "source");
}

#[test]
fn asset_index_text() {
    let (repo, _project) = setup_project_with_asset("alpha", "idx-asset.png");
    let output = mf(&repo).args(["asset", "index", "--project", "alpha"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("indexed asset:"), "stdout: {stdout}");
}

#[test]
fn asset_index_json() {
    let (repo, _project) = setup_project_with_asset("alpha", "idx-asset.png");
    let output = mf_json(&repo).args(["asset", "index", "--project", "alpha"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    let data = &v["data"];
    assert!(data.is_object());
    assert_eq!(data["kind"], "asset");
}

#[test]
fn project_index_text() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let output = mf(&repo).args(["project", "index"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("indexed project:"), "stdout: {stdout}");
}

#[test]
fn project_index_json() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let output = mf_json(&repo).args(["project", "index"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    let data = &v["data"];
    assert!(data.is_object());
    assert_eq!(data["kind"], "project");
}

// ═══════════════════════════════════════════════════════════════════════════════
// T062: Flag matrix — every mutating command has --dry-run; rename/remove have --force; remove/archive have --yes
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn rename_dry_run_all_resources() {
    // project rename --dry-run
    let repo = common::setup_repo();
    common::create_project(&repo, "proj1");
    let out = mf(&repo).args(["project", "rename", "proj1", "proj2", "--dry-run"]).output().unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8(out.stdout).unwrap().contains("[dry-run]"));

    // Create a source to rename with --dry-run
    let (repo2, _) = setup_project_with_source("beta", "s1");
    let out = mf(&repo2).args(["source", "rename", "s1", "s2", "--project", "beta", "--dry-run"]).output().unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8(out.stdout).unwrap().contains("[dry-run]"));

    // Create an asset to rename with --dry-run
    let (repo3, _) = setup_project_with_asset("gamma", "a1.png");
    let out =
        mf(&repo3).args(["asset", "rename", "a1.png", "a2.png", "--project", "gamma", "--dry-run"]).output().unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8(out.stdout).unwrap().contains("[dry-run]"));

    // term rename with --dry-run (has terms already)
    let repo4 = common::setup_repo();
    common::create_project(&repo4, "delta");
    let index_yaml = r#"schema_version: '1'
terms:
  - term: T1
    definition: d
    aliases: []
    tags: []
    corrections: []
"#;
    common::write_index(&repo4, "delta", index_yaml);
    let out = mf(&repo4).args(["term", "rename", "T1", "T2", "--project", "delta", "--dry-run"]).output().unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8(out.stdout).unwrap().contains("[dry-run]"));
}

#[test]
fn remove_dry_run_and_yes_flags() {
    let repo = common::setup_repo();
    common::create_project(&repo, "toremove");
    // --dry-run with --yes
    let out = mf(&repo).args(["project", "remove", "toremove", "--dry-run", "--yes"]).output().unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8(out.stdout).unwrap().contains("[dry-run]"));
    assert!(repo.path().join("toremove").exists());
}

#[test]
fn remove_force_flag_non_existent() {
    let repo = common::setup_repo();
    common::create_project(&repo, "exists");
    // --force with non-existent target should succeed (no-op)
    let out = mf(&repo).args(["project", "remove", "nonexistent", "--force"]).output().unwrap();
    assert!(out.status.success());
}

#[test]
fn index_dry_run_all_resources() {
    // project index --dry-run
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let out = mf(&repo).args(["project", "index", "--dry-run"]).output().unwrap();
    assert!(out.status.success());

    // source index --dry-run
    let (repo2, _) = setup_project_with_source("beta", "isrc");
    let out = mf(&repo2).args(["source", "index", "--project", "beta", "--dry-run"]).output().unwrap();
    assert!(out.status.success());

    // asset index --dry-run
    let (repo3, _) = setup_project_with_asset("gamma", "iasset.png");
    let out = mf(&repo3).args(["asset", "index", "--project", "gamma", "--dry-run"]).output().unwrap();
    assert!(out.status.success());
}
