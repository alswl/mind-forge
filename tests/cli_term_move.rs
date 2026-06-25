//! Regression coverage for `mf term move` conflict handling.
//!
//! A rejected move (destination already holds the term, no `--force`) must
//! fail WITHOUT deleting the source copy. Earlier the source was removed and
//! saved before the destination conflict was checked, silently losing the term.

use assert_cmd::Command;

mod common;

fn setup() -> common::TempDir {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    common::write_index(&repo, "alpha", "schema_version: '1'\n");
    repo
}

fn setup_two_projects() -> common::TempDir {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    common::create_project(&repo, "beta");
    common::write_index(&repo, "alpha", "schema_version: '1'\n");
    common::write_index(&repo, "beta", "schema_version: '1'\n");
    repo
}

fn mf(repo: &common::TempDir) -> Command {
    let mut c = Command::cargo_bin("mf").unwrap();
    c.args(["--root", repo.path().to_str().unwrap()]);
    c
}

fn project_index(repo: &common::TempDir, project: &str) -> String {
    std::fs::read_to_string(repo.path().join(project).join("mind-index.yaml")).unwrap_or_default()
}

fn global_terms(repo: &common::TempDir) -> String {
    std::fs::read_to_string(repo.path().join("minds-terms.yaml")).unwrap_or_default()
}

#[test]
fn project_to_global_conflict_preserves_source() {
    let repo = setup();
    // Same name in both the project and global scopes.
    mf(&repo).args(["--project", "alpha", "term", "new", "Overlap"]).output().unwrap();
    mf(&repo).args(["term", "new", "Overlap"]).output().unwrap();

    let output = mf(&repo).args(["--project", "alpha", "term", "move", "Overlap", "--to-global"]).output().unwrap();

    assert!(!output.status.success(), "move should fail on conflict");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("already exists"), "stderr: {stderr}");

    // Source copy must still be present.
    let index = std::fs::read_to_string(repo.path().join("alpha").join("mind-index.yaml")).unwrap();
    assert!(index.contains("Overlap"), "project term must survive a rejected move, got: {index}");
}

#[test]
fn global_to_project_conflict_preserves_source() {
    let repo = setup();
    mf(&repo).args(["--project", "alpha", "term", "new", "Overlap"]).output().unwrap();
    mf(&repo).args(["term", "new", "Overlap"]).output().unwrap();

    let output = mf(&repo)
        .args(["--project", "alpha", "term", "move", "Overlap", "--from-global", "--to-project", "alpha"])
        .output()
        .unwrap();

    assert!(!output.status.success(), "move should fail on conflict");

    // Global copy must still be present.
    let global = std::fs::read_to_string(repo.path().join("minds-terms.yaml")).unwrap();
    assert!(global.contains("Overlap"), "global term must survive a rejected move, got: {global}");
}

// ── Success paths ───────────────────────────────────────────────────────────

#[test]
fn project_to_global_moves_term() {
    let repo = setup();
    mf(&repo).args(["--project", "alpha", "term", "new", "Widget"]).output().unwrap();

    let output = mf(&repo).args(["--project", "alpha", "term", "move", "Widget", "--to-global"]).output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    assert!(!project_index(&repo, "alpha").contains("Widget"), "term should leave the project");
    assert!(global_terms(&repo).contains("Widget"), "term should land in global scope");
}

#[test]
fn global_to_project_moves_term() {
    let repo = setup();
    mf(&repo).args(["term", "new", "Widget"]).output().unwrap();

    let output = mf(&repo)
        .args(["--project", "alpha", "term", "move", "Widget", "--from-global", "--to-project", "alpha"])
        .output()
        .unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    assert!(!global_terms(&repo).contains("Widget"), "term should leave global scope");
    assert!(project_index(&repo, "alpha").contains("Widget"), "term should land in the project");
}

#[test]
fn project_to_project_moves_term() {
    let repo = setup_two_projects();
    mf(&repo).args(["--project", "alpha", "term", "new", "Widget"]).output().unwrap();

    let output =
        mf(&repo).args(["--project", "alpha", "term", "move", "Widget", "--to-project", "beta"]).output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    assert!(!project_index(&repo, "alpha").contains("Widget"), "term should leave the source project");
    assert!(project_index(&repo, "beta").contains("Widget"), "term should land in the destination project");
}

// ── Force overwrite ─────────────────────────────────────────────────────────

#[test]
fn force_overwrites_destination_conflict() {
    let repo = setup();
    // Project copy carries a definition; global copy does not.
    mf(&repo).args(["--project", "alpha", "term", "new", "Overlap", "--definition", "from project"]).output().unwrap();
    mf(&repo).args(["term", "new", "Overlap"]).output().unwrap();

    let output =
        mf(&repo).args(["--project", "alpha", "term", "move", "Overlap", "--to-global", "--force"]).output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    assert!(!project_index(&repo, "alpha").contains("Overlap"), "source should be gone after a forced move");
    let global = global_terms(&repo);
    assert!(global.contains("Overlap"), "destination should keep the term");
    assert!(global.contains("from project"), "forced move should overwrite with the source copy, got: {global}");
}

// ── Dry-run (T054) ──────────────────────────────────────────────────────────

#[test]
fn dry_run_writes_nothing() {
    let repo = setup();
    mf(&repo).args(["--project", "alpha", "term", "new", "Widget"]).output().unwrap();

    let output =
        mf(&repo).args(["--project", "alpha", "term", "move", "Widget", "--to-global", "--dry-run"]).output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    assert!(project_index(&repo, "alpha").contains("Widget"), "dry-run must not remove the source");
    assert!(!global_terms(&repo).contains("Widget"), "dry-run must not write the destination");
}

#[test]
fn dry_run_json_envelope() {
    let repo = setup();
    mf(&repo).args(["--project", "alpha", "term", "new", "Widget"]).output().unwrap();

    let output = mf(&repo)
        .args(["--project", "alpha", "--json", "term", "move", "Widget", "--to-global", "--dry-run"])
        .output()
        .unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_eq!(json["data"]["dry_run"], true);
    assert_eq!(json["data"]["details"]["from_scope"], "project");
    assert_eq!(json["data"]["details"]["to_scope"], "global");

    // Storage must still be unchanged
    assert!(project_index(&repo, "alpha").contains("Widget"), "dry-run must not remove the source");
}

// ── mv alias (T056) ─────────────────────────────────────────────────────────

#[test]
fn mv_alias_works_same_as_move() {
    let repo = setup();
    mf(&repo).args(["--project", "alpha", "term", "new", "Widget"]).output().unwrap();

    let output = mf(&repo).args(["--project", "alpha", "term", "mv", "Widget", "--to-global"]).output().unwrap();
    assert!(output.status.success(), "mv alias should work: {}", String::from_utf8_lossy(&output.stderr));

    assert!(!project_index(&repo, "alpha").contains("Widget"), "term should leave the project");
    assert!(global_terms(&repo).contains("Widget"), "term should land in global scope");
}

#[test]
fn mv_alias_json_identity() {
    let repo = setup();
    mf(&repo).args(["--project", "alpha", "term", "new", "Widget"]).output().unwrap();

    let output =
        mf(&repo).args(["--project", "alpha", "--json", "term", "mv", "Widget", "--to-global"]).output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_eq!(json["status"], "ok");
    assert_eq!(json["data"]["kind"], "term");
    assert_eq!(json["data"]["identity"], "Widget");
    assert_eq!(json["data"]["details"]["from_scope"], "project");
    assert_eq!(json["data"]["details"]["to_scope"], "global");
}
