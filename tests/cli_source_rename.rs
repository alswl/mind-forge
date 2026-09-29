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
  - name: notes
    type: file
    path: sources/file/notes.md
    tags: []
    added_at: '2026-05-08T11:00:00Z'
    updated_at: '2026-05-08T11:00:00Z'
  - name: research-blog
    type: web
    url: https://example.com/research
    tags: []
    added_at: '2026-05-08T12:00:00Z'
    updated_at: '2026-05-08T12:00:00Z'
"#;
    common::write_index(repo, project_name, yaml);
}

fn setup() -> (TempDir, std::path::PathBuf) {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    std::fs::create_dir_all(project.join("sources/pdf")).unwrap();
    std::fs::write(project.join("sources/pdf/paper.pdf"), b"fake pdf").unwrap();
    std::fs::create_dir_all(project.join("sources/file")).unwrap();
    std::fs::write(project.join("sources/file/notes.md"), b"# notes").unwrap();
    seed_sources(&repo, "alpha");
    (repo, project)
}

/// Run `mf source rename` from inside the project directory — the shape
/// every acceptance scenario in spec 082 US2 uses, and the one the target
/// path resolution rule (cwd-relative, checked against the project
/// boundary) is designed around.
fn rename_in_project(repo: &TempDir, project: &std::path::Path, args: &[&str]) -> std::process::Output {
    let mut full_args = vec!["--root", repo.path().to_str().unwrap(), "source", "rename"];
    full_args.extend_from_slice(args);
    full_args.extend_from_slice(&["--project", "alpha"]);
    Command::cargo_bin("mf").unwrap().args(&full_args).current_dir(project).output().unwrap()
}

// ---------------------------------------------------------------------------
// spec 082 US2 (#54): the target is a path, resolved cwd-relative and
// checked against the project boundary — not a bare name joined onto the
// old file's parent with its extension appended. This is a **deliberate
// breaking change** (spec FR-014): `mf source rename A B` with a
// separator-free `B` no longer means "rename in place, keep the extension".
// See CHANGELOG.md for the rewrite guidance these tests now follow.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// 1. rename_source_success — source renamed in index, target given as a
//    project-relative path (same directory, explicit extension)
// ---------------------------------------------------------------------------

#[test]
fn rename_source_success() {
    let (repo, project) = setup();

    let output = rename_in_project(&repo, &project, &["paper", "sources/pdf/whitepaper.pdf"]);

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(!index_content.contains("name: paper"), "old name should be gone, got: {index_content}");
    assert!(index_content.contains("name: whitepaper"), "new name should be present, got: {index_content}");
    assert!(project.join("sources/pdf/whitepaper.pdf").exists(), "file should exist at the target path");
    assert!(!project.join("sources/pdf/paper.pdf").exists(), "old file should be gone");
}

// ---------------------------------------------------------------------------
// 2. rename_source_file_renamed — on-disk file is also renamed
// ---------------------------------------------------------------------------

#[test]
fn rename_source_file_renamed() {
    let (repo, project) = setup();

    let output = rename_in_project(&repo, &project, &["notes", "sources/file/meeting-notes.md"]);

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    // Old file should not exist
    assert!(!project.join("sources/file/notes.md").exists(), "old file should be renamed");
    // New file should exist
    assert!(project.join("sources/file/meeting-notes.md").exists(), "new file should exist");
    // Index should reflect new name
    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("name: meeting-notes"), "new name should be in index");
}

// ---------------------------------------------------------------------------
// 3. rename_source_duplicate_refusal — a target already occupied by another
//    source's file fails without --force
// ---------------------------------------------------------------------------

#[test]
fn rename_source_duplicate_refusal() {
    let (repo, project) = setup();

    let output = rename_in_project(&repo, &project, &["paper", "sources/file/notes.md"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("already exists"), "stderr: {stderr}");
}

// ---------------------------------------------------------------------------
// 4. rename_source_dry_run — dry run does not mutate, and reports the same
//    destination the executing form would produce (FR-018)
// ---------------------------------------------------------------------------

#[test]
fn rename_source_dry_run() {
    let (repo, project) = setup();

    let output = rename_in_project(&repo, &project, &["notes", "sources/file/meeting-notes.md", "--dry-run"]);

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    // File should still be at old location
    assert!(project.join("sources/file/notes.md").exists(), "old file should still exist after dry run");
    // Index should still have old name
    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("name: notes"), "old name should still be in index");
    assert!(!index_content.contains("name: meeting-notes"), "new name should not appear");
}

// ---------------------------------------------------------------------------
// 5. rename_source_not_found — unknown source → usage error
// ---------------------------------------------------------------------------

#[test]
fn rename_source_not_found() {
    let (repo, project) = setup();

    let output = rename_in_project(&repo, &project, &["nonexistent", "sources/file/whatever.md"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not found"), "stderr: {stderr}");
}

// ---------------------------------------------------------------------------
// 6. rename_source_json_envelope — JSON output with full lifecycle envelope
// ---------------------------------------------------------------------------

#[test]
fn rename_source_json_envelope() {
    let (repo, project) = setup();

    let mut args = vec![
        "--root",
        repo.path().to_str().unwrap(),
        "source",
        "rename",
        "paper",
        "sources/pdf/whitepaper.pdf",
        "--project",
        "alpha",
        "--output",
        "json",
    ];
    let output = Command::cargo_bin("mf").unwrap().args(&mut args).current_dir(&project).output().unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    assert_eq!(v["data"]["kind"], "source");
    assert_eq!(v["data"]["old_identity"], "paper");
    assert_eq!(v["data"]["identity"], "whitepaper");
    assert_eq!(v["data"]["dry_run"], false);
}

#[test]
fn rename_source_path_target_lands_exactly_no_duplication() {
    let (repo, project) = setup();
    std::fs::create_dir_all(project.join("sources/yuque")).unwrap();

    let output = rename_in_project(&repo, &project, &["notes", "sources/yuque/renamed.md"]);
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    assert!(
        project.join("sources/yuque/renamed.md").exists(),
        "file must land at exactly the target path, no duplicated segment"
    );
    assert!(!project.join("sources/file/sources/yuque/renamed.md").exists());
    assert!(!project.join("sources/yuque/renamed.md.md").exists(), "extension must not be doubled");

    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("path: sources/yuque/renamed.md"), "index path: {index_content}");
    assert!(index_content.contains("name: renamed"), "index name: {index_content}");
}

#[test]
fn rename_source_bare_name_no_longer_corrupts_the_registration() {
    let (repo, project) = setup();

    let output = rename_in_project(&repo, &project, &["notes", "renamed.md"]);
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    assert!(
        !project.join("sources/file/renamed.md.md").exists(),
        "the historical corruption (doubled extension) must not reproduce"
    );
    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(
        !index_content.contains("name: renamed.md"),
        "the registered name must never be the literal string 'renamed.md': {index_content}"
    );
    assert!(index_content.contains("name: renamed"), "index: {index_content}");
    assert!(project.join("renamed.md").exists(), "cwd-relative bare name resolves under the project root");
}

#[test]
fn rename_source_target_parent_missing_is_refused() {
    let (repo, project) = setup();

    let output = rename_in_project(&repo, &project, &["notes", "sources/missing-dir/x.md"]);
    assert!(!output.status.success(), "must refuse when the parent directory does not exist");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("missing-dir") || stderr.contains("not found"), "stderr should name the problem: {stderr}");

    assert!(project.join("sources/file/notes.md").exists(), "source file must be untouched");
    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("name: notes"), "index must be untouched: {index_content}");
}

#[test]
fn rename_source_target_outside_project_is_refused() {
    let (repo, project) = setup();
    // cwd == project, but the target's absolute form falls outside it.
    let outside = repo.path().join("outside-target.md");
    let output = rename_in_project(&repo, &project, &["notes", outside.to_str().unwrap()]);
    assert!(!output.status.success(), "must refuse a target outside the project boundary");

    assert!(!outside.exists(), "nothing should be written outside the project");
    assert!(project.join("sources/file/notes.md").exists(), "source file must be untouched");
}

#[test]
fn rename_source_target_with_dotdot_is_refused() {
    let (repo, project) = setup();

    let output = rename_in_project(&repo, &project, &["notes", "../escape.md"]);
    assert!(!output.status.success(), "'..' must be refused on every path parameter, not just --project");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains(".."), "stderr should name the restriction: {stderr}");

    assert!(project.join("sources/file/notes.md").exists(), "source file must be untouched");
}

#[test]
fn rename_source_matrix_never_surfaces_internal_error() {
    let (repo, project) = setup();

    let cases: &[&[&str]] = &[
        &["notes", "sources/missing-dir/x.md"],
        &["notes", "../escape.md"],
        &["nonexistent-source", "sources/file/whatever.md"],
    ];
    for args in cases {
        let mut full_args = vec!["--root", repo.path().to_str().unwrap(), "--json", "source", "rename"];
        full_args.extend_from_slice(args);
        full_args.extend_from_slice(&["--project", "alpha"]);
        let output = Command::cargo_bin("mf").unwrap().args(&full_args).current_dir(&project).output().unwrap();
        assert!(!output.status.success(), "expected failure for {args:?}");
        let stdout = String::from_utf8(output.stdout).unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        let error_output = if stderr.trim().is_empty() { stdout } else { stderr };
        let v: serde_json::Value = serde_json::from_str(&error_output).unwrap_or_else(|e| {
            panic!("expected JSON error envelope for {args:?}, got parse error {e}: {error_output}")
        });
        let kind = v["error"]["kind"].as_str().unwrap_or_default();
        assert_ne!(kind, "internal", "must not surface an internal error for {args:?}: {v}");
        let message = v["error"]["message"].as_str().unwrap_or_default();
        assert!(!message.contains("os error"), "must not surface a bare OS error string for {args:?}: {message}");
    }
}

#[test]
fn rename_source_dry_run_reports_true_destination() {
    let (repo, project) = setup();

    let mut full_args = vec![
        "--root",
        repo.path().to_str().unwrap(),
        "--json",
        "source",
        "rename",
        "notes",
        "sources/yuque/renamed.md",
        "--dry-run",
        "--project",
        "alpha",
    ];
    std::fs::create_dir_all(project.join("sources/yuque")).unwrap();
    let output = Command::cargo_bin("mf").unwrap().args(&mut full_args).current_dir(&project).output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let dry: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let real_output = rename_in_project(&repo, &project, &["notes", "sources/yuque/renamed.md"]);
    assert!(real_output.status.success());

    assert_eq!(dry["data"]["identity"], "renamed", "dry-run should report the same eventual name: {dry}");
    assert!(project.join("sources/yuque/renamed.md").exists());
}

#[test]
fn rename_source_url_only_registration_updates_name_without_file_ops() {
    let (repo, project) = setup();

    let output = rename_in_project(&repo, &project, &["research-blog", "sources/web/renamed-blog"]);
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("name: renamed-blog"), "index: {index_content}");
    assert!(index_content.contains("url: https://example.com/research"), "url must be preserved: {index_content}");
    assert!(!project.join("sources/web/renamed-blog").exists(), "no file should be created for a URL-only source");
}
