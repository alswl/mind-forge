use std::fs;

use crate::datasets::*;
use crate::helpers::*;

// ═══════════════════════════════════════════════════════════════════════════════
// T118: list → identity → show → modify chain across two resources
// ═══════════════════════════════════════════════════════════════════════════════

/// E2E: project list → show using identity → rename → show renamed
#[test]
fn e2e_project_chain_list_show_rename_show() {
    let ds = repo_008_with_data();

    // 1. List projects and extract identity from JSON
    let (stdout, _, code) = run_in(ds.root(), &["project", "list", "--json"]);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let projects = v["data"]["projects"].as_array().expect("projects array");
    assert!(!projects.is_empty(), "should have at least one project");
    let identity = projects[0]["identity"].as_str().expect("identity field");
    assert!(!identity.is_empty());

    // 2. Show the project using identity from list
    let (stdout, _, code) = run_in(ds.root(), &["project", "show", identity]);
    assert_eq!(code, 0, "should show project by identity: {stdout}");

    // 3. Show via JSON — verify identity round-trip
    let (stdout, _, code) = run_in(ds.root(), &["project", "show", identity, "--json"]);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(v["data"]["identity"].as_str().unwrap(), identity);

    // 4. Rename the project
    let new_name = format!("{identity}-renamed");
    let (stdout, _, code) = run_in(ds.root(), &["project", "rename", identity, &new_name]);
    assert_eq!(code, 0, "should rename: {stdout}");
    assert!(stdout.contains("renamed project:"), "stdout: {stdout}");

    // 5. Show renamed project
    let (stdout, _, code) = run_in(ds.root(), &["project", "show", &new_name]);
    assert_eq!(code, 0, "should show renamed project: {stdout}");
}

/// E2E: article list → show using identity → show json round-trip
#[test]
fn e2e_article_chain_list_show_identity_roundtrip() {
    let ds = Dataset::empty().with_standard_project("alpha").with_index(
        "alpha",
        r#"schema_version: '1'
articles:
  - title: "Test Post"
    project: "alpha"
    type: blog
    article_path: "docs/test-post.md"
    status: draft
    created_at: "2026-05-20T00:00:00Z"
    updated_at: "2026-05-20T00:00:00Z"
"#,
    );

    // Register the project in the manifest
    let manifest = "schema_version: '1'\nprojects:\n  - name: alpha\n    path: ./projects/alpha\n    created_at: \"2026-05-20T00:00:00Z\"\n    archived_at: ~\n".to_string();
    fs::write(ds.root().join("minds.yaml"), &manifest).expect("write manifest");

    // Create the docs file
    fs::create_dir_all(ds.root().join("projects/alpha/docs")).unwrap();
    fs::write(ds.root().join("projects/alpha/docs/test-post.md"), "# Test Post\n").unwrap();

    // 1. List articles and extract identity from JSON
    let (stdout, _, code) = run_in(ds.root(), &["-p", "alpha", "article", "list", "--json"]);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let articles = v["data"]["articles"].as_array().expect("articles array");
    assert!(!articles.is_empty(), "should have at least one article: {stdout}");
    let identity = articles[0]["identity"].as_str().expect("identity field");
    assert_eq!(identity, "docs/test-post.md");

    // 2. Show article using identity from list
    let (stdout, _, code) = run_in(ds.root(), &["-p", "alpha", "article", "show", identity]);
    assert_eq!(code, 0, "should show article by identity: {stdout}");

    // 3. Show via JSON — verify identity round-trip
    let (stdout, _, code) = run_in(ds.root(), &["-p", "alpha", "article", "show", identity, "--json"]);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(v["data"]["identity"].as_str().unwrap(), identity);
}

/// E2E: term list → show using identity → JSON round-trip
#[test]
fn e2e_term_chain_list_show_identity_roundtrip() {
    let ds = Dataset::empty().with_standard_project("alpha").with_index(
        "alpha",
        r#"schema_version: '1'
terms:
  - term: "Zettelkasten"
    definition: "A note-taking method"
    aliases: []
    tags: []
    confidence: 0.9
    created_at: "2026-05-20T00:00:00Z"
    updated_at: "2026-05-20T00:00:00Z"
"#,
    );

    // Register the project in the manifest
    let manifest = "schema_version: '1'\nprojects:\n  - name: alpha\n    path: ./projects/alpha\n    created_at: \"2026-05-20T00:00:00Z\"\n    archived_at: ~\n".to_string();
    fs::write(ds.root().join("minds.yaml"), &manifest).expect("write manifest");

    // 1. List terms and extract identity from JSON
    let (stdout, stderr, code) = run_in(ds.root(), &["-p", "alpha", "term", "list", "--json"]);
    assert_eq!(code, 0, "stdout: {stdout}, stderr: {stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let terms = v["data"]["terms"].as_array().expect("terms array");
    assert!(!terms.is_empty(), "should have at least one term: {stdout}");
    let identity = terms[0]["identity"].as_str().expect("identity field");
    assert_eq!(identity, "Zettelkasten");

    // 2. Show term using identity from list
    let (stdout, _, code) = run_in(ds.root(), &["-p", "alpha", "term", "show", identity]);
    assert_eq!(code, 0, "should show term by identity: {stdout}");

    // 3. Show via JSON — verify identity round-trip
    let (stdout, _, code) = run_in(ds.root(), &["-p", "alpha", "term", "show", identity, "--json"]);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(v["data"]["identity"].as_str().unwrap(), identity);
}

/// E2E: cross-resource chain: project → create article → list → show → index
#[test]
fn e2e_cross_resource_chain_project_to_article() {
    let ds = Dataset::empty();

    // 1. Create a project
    let (stdout, _, code) = run_in(ds.root(), &["project", "new", "blog"]);
    assert_eq!(code, 0, "should create project: {stdout}");

    // 2. Create an article in that project
    let (stdout, _, code) = run_in(ds.root(), &["article", "new", "Hello World", "--project", "blog", "--file"]);
    assert_eq!(code, 0, "should create article: {stdout}");

    // 3. List articles and verify identity round-trip
    let (stdout, _, code) = run_in(ds.root(), &["-p", "blog", "article", "list", "--json"]);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let articles = v["data"]["articles"].as_array().expect("articles array");
    assert!(!articles.is_empty(), "should have at least one article: {stdout}");
    let identity = articles[0]["identity"].as_str().expect("identity field");

    // 4. Show article using identity
    let (stdout, _, code) = run_in(ds.root(), &["-p", "blog", "article", "show", identity]);
    assert_eq!(code, 0, "should show article: {stdout}");

    // 5. Index articles
    let (stdout, _, code) = run_in(ds.root(), &["-p", "blog", "article", "index"]);
    assert_eq!(code, 0, "should index articles: {stdout}");
    assert!(stdout.contains("indexed article"), "stdout: {stdout}");
}
