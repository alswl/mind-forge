//! T048 — US3 contracts for project→global silent-drop fall-through.
//!
//! When a `-p <project>` write targets a name that only exists in the global
//! pool, the CLI must:
//!   1. Apply the write to the global scope (instead of erroring).
//!   2. Emit a single `WARN: -p <project> was ignored; ...` line on stderr.
//!   3. Surface the same line in the JSON envelope's `data.warnings`.
//!   4. Exit 0 on success.
//!
//! Reads (`term show`) follow the same fall-through rule but without a write,
//! so they tag the JSON envelope with `scope: "global"` instead of warning.

use assert_cmd::Command;

mod common;

fn setup() -> common::TempDir {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    common::write_index(&repo, "alpha", "schema_version: '1'\n");
    repo
}

fn mf(repo: &common::TempDir) -> Command {
    let mut c = Command::cargo_bin("mf").unwrap();
    c.args(["--root", repo.path().to_str().unwrap()]);
    c
}

fn seed_global_term(repo: &common::TempDir, name: &str) {
    let output = mf(repo).args(["term", "new", name]).output().unwrap();
    assert!(output.status.success(), "seed failed: {:?}", String::from_utf8_lossy(&output.stderr));
}

// ── term show ──────────────────────────────────────────────────────────────

#[test]
fn show_falls_through_to_global_when_project_lacks_term() {
    let repo = setup();
    seed_global_term(&repo, "Kubernetes");

    let output = mf(&repo).args(["--project", "alpha", "--json", "term", "show", "Kubernetes"]).output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let v: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_eq!(v["status"], "ok");
    assert_eq!(v["data"]["scope"], "global", "expected scope=global, got: {}", v["data"]);
}

#[test]
fn show_text_mode_displays_scope_global() {
    let repo = setup();
    seed_global_term(&repo, "Kubernetes");

    let output = mf(&repo).args(["--project", "alpha", "term", "show", "Kubernetes"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Scope:"), "expected Scope row in text mode: {stdout}");
    assert!(stdout.contains("global"), "expected global scope label: {stdout}");
}

#[test]
fn show_missing_in_both_scopes_errors() {
    let repo = setup();
    // Nothing seeded — show should fail.
    let output = mf(&repo).args(["--project", "alpha", "term", "show", "Ghost"]).output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not found"), "stderr should name the missing term: {stderr}");
}

// ── term update ────────────────────────────────────────────────────────────

#[test]
fn update_falls_through_to_global_with_warn() {
    let repo = setup();
    seed_global_term(&repo, "Kubernetes");

    let output =
        mf(&repo).args(["--project", "alpha", "term", "update", "Kubernetes", "--tag", "k8s"]).output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("WARN:") && stderr.contains("-p alpha was ignored") && stderr.contains("global scope"),
        "stderr must carry the silent-drop WARN: {stderr}"
    );
}

#[test]
fn update_json_envelope_includes_warning() {
    let repo = setup();
    seed_global_term(&repo, "Kubernetes");

    let output = mf(&repo)
        .args(["--project", "alpha", "--json", "term", "update", "Kubernetes", "--tag", "k8s"])
        .output()
        .unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let v: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    let warnings = v["data"]["warnings"].as_array().expect("data.warnings should be an array");
    assert!(!warnings.is_empty(), "warnings must be non-empty: {}", v["data"]);
    assert!(warnings.iter().any(|w| w.as_str().unwrap_or("").contains("-p alpha was ignored")));
}

#[test]
fn update_missing_in_both_scopes_does_not_emit_warn() {
    let repo = setup();
    // Nothing seeded.
    let output = mf(&repo).args(["--project", "alpha", "term", "update", "Ghost", "--tag", "x"]).output().unwrap();
    assert!(!output.status.success(), "must fail when missing everywhere");

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        !stderr.contains("applied to global scope"),
        "must NOT lie about applying to global when global also missing: {stderr}"
    );
}

// ── term remove ────────────────────────────────────────────────────────────

#[test]
fn remove_falls_through_to_global_with_warn() {
    let repo = setup();
    seed_global_term(&repo, "Kubernetes");

    let output = mf(&repo).args(["--project", "alpha", "term", "remove", "Kubernetes", "--yes"]).output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("WARN:") && stderr.contains("-p alpha was ignored"),
        "remove silent-drop WARN missing: {stderr}"
    );

    // Verify the term is actually gone from global.
    let post = std::fs::read_to_string(repo.path().join("minds-terms.yaml")).unwrap_or_default();
    assert!(!post.contains("Kubernetes"), "global term should be removed: {post}");
}

// ── term rename ────────────────────────────────────────────────────────────

#[test]
fn rename_falls_through_to_global_with_warn() {
    let repo = setup();
    seed_global_term(&repo, "Kubernetes");

    let output =
        mf(&repo).args(["--project", "alpha", "term", "rename", "Kubernetes", "K8s", "--force"]).output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("WARN:") && stderr.contains("-p alpha was ignored"),
        "rename silent-drop WARN missing: {stderr}"
    );

    // Verify the rename actually landed in global.
    let post = std::fs::read_to_string(repo.path().join("minds-terms.yaml")).unwrap_or_default();
    assert!(post.contains("K8s") && !post.contains("Kubernetes"), "global rename should land: {post}");
}

// ── term list -p with global merge ─────────────────────────────────────────

#[test]
fn list_merges_global_terms_with_scope_tag_in_json() {
    let repo = setup();
    seed_global_term(&repo, "Kubernetes");
    // Add a project-only term too.
    let _ = mf(&repo).args(["--project", "alpha", "term", "new", "ProjectOnly"]).output().unwrap();

    let output = mf(&repo).args(["--project", "alpha", "--json", "term", "list"]).output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let v: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    let terms = v["data"]["terms"].as_array().expect("data.terms array");

    let mut by_name: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    for t in terms {
        let name = t["term"].as_str().unwrap();
        let scope = t["scope"].as_str().unwrap_or("");
        by_name.insert(name, scope);
    }
    assert_eq!(by_name.get("Kubernetes"), Some(&"global"), "expected Kubernetes tagged global: {by_name:?}");
    assert_eq!(by_name.get("ProjectOnly"), Some(&"project"), "expected ProjectOnly tagged project: {by_name:?}");
}
