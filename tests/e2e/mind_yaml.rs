//! E2E scenarios for Mind YAML Compatibility (017-mind-yaml).
//!
//! These scenarios exercise the acceptance command set against the
//! deterministic compatibility fixture repo.

use crate::datasets::mind_yaml_compat_repo;
use crate::helpers::*;

/// US1 (P1 MVP): `mf project list --json` reads a `minds.yaml` whose
/// `projects` are path strings, exits 0, returns JSON status `ok`, lists
/// all 12 fixture projects, and leaves fixture YAML unchanged.
#[test]
fn e2e_mind_yaml_project_list_reads_string_manifest() {
    let ds = mind_yaml_compat_repo();

    let (stdout, stderr, code) = run_in(ds.root(), &["project", "list", "--json"]);
    assert_eq!(code, 0, "project list --json failed: {stderr}");
    assert!(stderr.is_empty(), "expected clean stderr: {stderr}");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(value["status"], "ok", "envelope status is ok: {stdout}");

    // All 12 projects should be listed
    let projects = value["data"]["projects"].as_array().expect("data.projects is an array");
    assert_eq!(projects.len(), 12, "should list 12 projects");

    // Verify some key projects are present
    let names: Vec<&str> = projects.iter().filter_map(|v| v["name"].as_str()).collect();
    assert!(names.contains(&"2026-blogs"), "should include 2026-blogs");
    assert!(names.contains(&"2026-meetings"), "should include 2026-meetings");
    assert!(names.contains(&"team-reports"), "should include team-reports");
    assert!(names.contains(&"2026-hcs-tickets"), "should include 2026-hcs-tickets");

    // Verify fixture files were NOT rewritten
    let minds_yaml = std::fs::read_to_string(ds.root().join("minds.yaml")).expect("read minds.yaml");
    assert!(minds_yaml.contains("projects:"), "minds.yaml still has projects field");
    // Should not contain the full object syntax (name: / path:)
    assert!(!minds_yaml.contains("name: 2026-blogs"), "minds.yaml was not rewritten to object format");
}

/// US2: `mf project status --json` and `mf project lint --json` work
/// against the variant mind.yaml files.
#[test]
fn e2e_mind_yaml_project_status_and_lint_accept_existing_configs() {
    let ds = mind_yaml_compat_repo();

    // Empty mind.yaml (2026-hcs-tickets) - should use defaults
    let (stdout, stderr, code) = run_in(ds.root(), &["project", "status", "--project", "2026-hcs-tickets", "--json"]);
    assert_eq!(code, 0, "status empty mind.yaml failed: {stderr}");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(value["status"], "ok", "envelope status is ok for empty mind.yaml");

    // Top-level name/description (2026-meetings)
    let (stdout, stderr, code) = run_in(ds.root(), &["project", "status", "--project", "2026-meetings", "--json"]);
    assert_eq!(code, 0, "status top-level metadata failed: {stderr}");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(value["status"], "ok");
    assert_eq!(value["data"]["name"], "2026 Meetings");

    // Wrapped project + build/templates/publish (team-reports)
    let (stdout, stderr, code) = run_in(ds.root(), &["project", "status", "--project", "team-reports", "--json"]);
    assert_eq!(code, 0, "status team-reports failed: {stderr}");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(value["status"], "ok");

    // Build + yuque_cc (2026-blogs)
    let (stdout, stderr, code) = run_in(ds.root(), &["project", "status", "--project", "2026-blogs", "--json"]);
    assert_eq!(code, 0, "status 2026-blogs failed: {stderr}");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(value["status"], "ok");

    // 2026-hcs-tickets (empty mind.yaml)
    let (stdout, stderr, code) = run_in(ds.root(), &["project", "status", "--project", "2026-hcs-tickets", "--json"]);
    assert_eq!(code, 0, "status 2026-hcs-tickets failed: {stderr}");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(value["status"], "ok");

    // Lint should recognize all known publish target shapes without
    // compatibility parse errors. Genuine lint issues (missing dirs, stale
    // entries) may produce exit code != 0, but the JSON envelope must be "ok".
    let (stdout, stderr, _code) = run_in(ds.root(), &["project", "lint", "--json"]);
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(value["status"], "ok", "lint JSON envelope should be ok: {stderr}");

    // Verify fixture files were NOT modified
    let empty_mind =
        std::fs::read_to_string(ds.root().join("projects/2026-hcs-tickets/mind.yaml")).expect("read mind.yaml");
    assert!(empty_mind.trim().is_empty(), "empty mind.yaml should remain empty");
}

/// US3: Article, source, and asset list commands accept dictionary-based
/// `mind-index.yaml` files.
#[test]
fn e2e_mind_yaml_index_dictionary_commands_complete() {
    let ds = mind_yaml_compat_repo();

    // Article list against dictionary index
    let (stdout, stderr, code) = run_in(ds.root(), &["article", "list", "--project", "2026-blogs", "--json"]);
    assert_eq!(code, 0, "article list failed: {stderr}");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(value["status"], "ok");
    assert_eq!(value["data"]["articles"].as_array().expect("articles array").len(), 3);

    // Source list against path-keyed dictionary index
    let (stdout, stderr, code) = run_in(ds.root(), &["source", "list", "--project", "2026-ai-sites-build", "--json"]);
    assert_eq!(code, 0, "source list failed: {stderr}");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(value["status"], "ok");
    let sources = value["data"]["sources"].as_array().expect("sources array");
    assert!(sources.iter().any(|source| source["path"] == "docs/api-spec.md"));

    // Source list also accepts legacy files dictionaries as source entries.
    let (stdout, stderr, code) = run_in(ds.root(), &["source", "list", "--project", "2026-blogs", "--json"]);
    assert_eq!(code, 0, "source list files dictionary failed: {stderr}");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(value["status"], "ok");
    let sources = value["data"]["sources"].as_array().expect("sources array");
    assert!(sources.iter().any(|source| source["path"] == "docs/getting-started-with-rust.md"));

    // Asset list against filename-keyed dictionary index
    let (stdout, stderr, code) = run_in(ds.root(), &["asset", "list", "--project", "2026-03-hid-prd", "--json"]);
    assert_eq!(code, 0, "asset list failed: {stderr}");
    let value: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(value["status"], "ok");
    assert!(value["data"]["assets"].as_array().expect("assets array").iter().any(|asset| asset["name"] == "logo.png"));
}

/// Read-only commands must NOT modify fixture YAML files.
#[test]
fn e2e_mind_yaml_read_only_does_not_modify_fixtures() {
    let ds = mind_yaml_compat_repo();

    // Record checksums of all YAML files before any command runs
    let files: &[&str] = &[
        "minds.yaml",
        "projects/2026-hcs-tickets/mind.yaml",
        "projects/2026-meetings/mind.yaml",
        "projects/2026-blogs/mind.yaml",
        "projects/team-reports/mind.yaml",
        "projects/team-reports/mind-index.yaml",
        "projects/2026-blogs/mind-index.yaml",
        "projects/2026-ai-sites-build/mind-index.yaml",
        "projects/2026-03-hid-prd/mind-index.yaml",
    ];
    let checksums: Vec<(String, String)> = files
        .iter()
        .map(|f| {
            let path = ds.root().join(f);
            let content = std::fs::read_to_string(&path).unwrap_or_default();
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            content.hash(&mut hasher);
            (f.to_string(), hasher.finish().to_string())
        })
        .collect();

    // Run read-only commands
    run_in(ds.root(), &["project", "list", "--json"]);
    run_in(ds.root(), &["project", "status", "--project", "2026-blogs", "--json"]);
    run_in(ds.root(), &["project", "status", "--project", "2026-meetings", "--json"]);
    run_in(ds.root(), &["project", "status", "--project", "team-reports", "--json"]);
    run_in(ds.root(), &["project", "status", "--project", "2026-hcs-tickets", "--json"]);
    run_in(ds.root(), &["article", "list", "--project", "2026-blogs", "--json"]);
    run_in(ds.root(), &["source", "list", "--project", "2026-ai-sites-build", "--json"]);
    run_in(ds.root(), &["asset", "list", "--project", "2026-03-hid-prd", "--json"]);
    run_in(ds.root(), &["project", "lint", "--json"]);

    // Verify checksums match
    for (filename, original_checksum) in &checksums {
        let path = ds.root().join(filename);
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        content.hash(&mut hasher);
        let new_checksum = hasher.finish().to_string();
        assert_eq!(&new_checksum, original_checksum, "file was modified by read-only commands: {filename}");
    }
}
