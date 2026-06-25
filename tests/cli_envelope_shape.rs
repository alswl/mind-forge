use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;

mod common;

// ===== Shared envelope assertion helpers =====

const ENVELOPE_COMMAND: &str = "mf";

fn assert_envelope_ok(json: &str) {
    let v: Value = serde_json::from_str(json).expect("stdout should be valid JSON");
    assert_eq!(v["status"], "ok", "envelope status should be 'ok': {json}");
    assert_eq!(v["command"].as_str().unwrap_or_default(), ENVELOPE_COMMAND, "envelope command should match: {json}");
    assert!(v.get("data").is_some() && !v["data"].is_null(), "envelope data should be present and non-null: {json}");
}

fn assert_envelope_err(json: &str) {
    let v: Value = serde_json::from_str(json).expect("stdout should be valid JSON");
    assert_eq!(v["status"], "error", "envelope status should be 'error': {json}");
    assert_eq!(v["command"].as_str().unwrap_or_default(), ENVELOPE_COMMAND, "envelope command should match: {json}");
    assert!(v["error"]["kind"].as_str().is_some(), "error.kind should be a non-null string: {json}");
    assert!(v["error"]["message"].is_string(), "error.message should be a string: {json}");
}

fn run(args: &[&str]) -> (String, String, i32) {
    let output = Command::cargo_bin("mf").expect("binary exists").args(args).output().expect("command runs");
    (
        String::from_utf8(output.stdout).expect("stdout utf8"),
        String::from_utf8(output.stderr).expect("stderr utf8"),
        output.status.code().unwrap_or_default(),
    )
}

fn run_json(args: &[&str]) -> (String, String, i32) {
    let mut full_args = vec!["--output", "json"];
    full_args.extend_from_slice(args);
    run(&full_args)
}

// ===== Model-level extraction helpers =====

/// Try to parse stdout as a JSON envelope and return the `data` portion.
/// For success cases this should always succeed.
fn extract_data(stdout: &str) -> Value {
    let v: Value = serde_json::from_str(stdout).expect("stdout should be valid JSON");
    v["data"].clone()
}

/// Parse error envelope from JSON stdout.
fn extract_error(stdout: &str) -> Value {
    let v: Value = serde_json::from_str(stdout).expect("stdout should be valid JSON");
    assert_eq!(v["status"], "error", "expected error envelope, got: {stdout}");
    v["error"].clone()
}

// ===== Setup helpers =====

fn setup_project(repo: &TempDir, name: &str) {
    common::create_project(repo, name);
    let project = repo.path().join(name);
    std::fs::create_dir_all(project.join("assets")).unwrap();
}

fn add_asset_entry(repo: &TempDir, project: &str, asset_name: &str, on_disk: bool) {
    let project_dir = repo.path().join(project);
    if on_disk {
        std::fs::write(project_dir.join("assets").join(asset_name), b"content").unwrap();
    }
    let index_path = project_dir.join("mind-index.yaml");
    let mut yaml = std::fs::read_to_string(&index_path).unwrap_or_default();
    if yaml.trim().is_empty() {
        yaml = "schema_version: '1'\n".to_string();
    }
    let entry = format!(
        r#"  - name: "{asset_name}"
    type: image
    path: "assets/{asset_name}"
    size: 7
    hash: "abc123"
    tags: []
    added_at: "2026-05-01T00:00:00Z"
"#
    );
    if yaml.contains("assets:") {
        let marker = if yaml.contains("\narticles:") {
            "\narticles:"
        } else if yaml.contains("\nsources:") {
            "\nsources:"
        } else {
            "\nterms:"
        };
        if yaml.contains(marker) {
            yaml = yaml.replace(marker, &format!("{entry}{marker}"));
        } else {
            yaml = format!("{yaml}{entry}");
        }
    } else {
        yaml = format!("{}assets:\n{}", yaml, entry);
    }
    std::fs::write(&index_path, &yaml).unwrap();
}

fn setup_with_term(repo: &TempDir) {
    common::create_project(repo, "alpha");
    let index_yaml = r#"schema_version: '1'
terms:
  - term: API
    definition: Application Programming Interface
    aliases:
      - application-api
    tags:
      - tech
    corrections:
      - original: ap-i
        correct: API
"#;
    common::write_index(repo, "alpha", index_yaml);
}

// ==========================================================================
// B1.1: mf asset clean — envelope shape
// ==========================================================================

#[test]
fn envelope_asset_clean_ok() {
    let repo = common::setup_repo();
    setup_project(&repo, "test");
    add_asset_entry(&repo, "test", "exists.png", true);
    add_asset_entry(&repo, "test", "missing.png", false);

    let (stdout, stderr, code) =
        run_json(&["--root", &repo.path().to_string_lossy(), "asset", "clean", "--project", "test"]);
    assert_eq!(code, 0, "stderr: {stderr:?}");
    assert_envelope_ok(&stdout);

    let data = extract_data(&stdout);
    assert!(data["stale_entries"].is_array(), "stale_entries should be array");
    assert_eq!(data["removed_count"], 1, "should have removed 1 stale entry");
    assert_eq!(data["dry_run"], false, "dry_run should be false");
}

#[test]
fn envelope_asset_clean_dry_run() {
    let repo = common::setup_repo();
    setup_project(&repo, "test");
    add_asset_entry(&repo, "test", "missing.png", false);

    let (stdout, stderr, code) =
        run_json(&["--root", &repo.path().to_string_lossy(), "asset", "clean", "--project", "test", "--dry-run"]);
    assert_eq!(code, 0, "stderr: {stderr:?}");
    assert_envelope_ok(&stdout);

    let data = extract_data(&stdout);
    assert_eq!(data["dry_run"], true, "dry_run should be true");
}

// ==========================================================================
// B1.2: mf asset remove — envelope shape
// ==========================================================================

#[test]
fn envelope_asset_remove_ok() {
    let repo = common::setup_repo();
    setup_project(&repo, "test");
    add_asset_entry(&repo, "test", "logo.png", true);

    let (stdout, stderr, code) = run_json(&[
        "--root",
        &repo.path().to_string_lossy(),
        "asset",
        "remove",
        "logo.png",
        "--project",
        "test",
        "--yes",
    ]);
    assert_eq!(code, 0, "stderr: {stderr:?}");
    assert_envelope_ok(&stdout);

    let data = extract_data(&stdout);
    assert_eq!(data["removed"], true, "removed should be true");
    assert_eq!(data["kind"], "asset", "kind should be asset");
}

#[test]
fn envelope_asset_remove_referenced_error() {
    let repo = common::setup_repo();
    setup_project(&repo, "test");
    let project_dir = repo.path().join("test");
    std::fs::write(project_dir.join("assets/logo.png"), b"content").unwrap();
    // Create the article article file that references the asset
    let doc_dir = project_dir.join("docs");
    std::fs::create_dir_all(&doc_dir).unwrap();
    std::fs::write(doc_dir.join("welcome.md"), b"uses logo.png").unwrap();
    let index_yaml = r#"schema_version: '1'
assets:
  - name: "logo.png"
    type: image
    path: "assets/logo.png"
    size: 7
    hash: "abc123"
    tags: []
    added_at: "2026-05-01T00:00:00Z"
articles:
  - title: "welcome"
    project: "test"
    type: blog
    article_path: "docs/welcome.md"
    status: draft
    created_at: "2026-05-01T00:00:00Z"
    updated_at: "2026-05-01T00:00:00Z"
"#;
    std::fs::write(project_dir.join("mind-index.yaml"), index_yaml).unwrap();

    let (stdout, stderr, code) = run_json(&[
        "--root",
        &repo.path().to_string_lossy(),
        "asset",
        "remove",
        "logo.png",
        "--project",
        "test",
        "--yes",
    ]);
    assert_eq!(code, 2, "should error when referenced, stderr: {stderr:?}");
    // Error may be on stderr, but with --output json it's on stdout
    if stdout.trim().is_empty() {
        assert!(!stderr.is_empty(), "stderr should have error message");
    } else {
        assert_envelope_err(&stdout);
        let err = extract_error(&stdout);
        assert!(err["kind"].as_str().is_some());
    }
}

// ==========================================================================
// B1.3: mf project show — envelope shape
// ==========================================================================

#[test]
fn envelope_project_show_ok() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let (stdout, stderr, code) = run_json(&["--root", &repo.path().to_string_lossy(), "project", "show", "my-project"]);
    assert_eq!(code, 0, "stderr: {stderr:?}");
    assert_envelope_ok(&stdout);

    let data = extract_data(&stdout);
    assert_eq!(data["name"], "my-project");
    assert!(data["path"].is_string());
    assert!(data["article_count"].is_number(), "should have article_count");
    assert!(data["source_count"].is_number(), "should have source_count");
    assert!(data["asset_count"].is_number(), "should have asset_count");
    assert!(data["mind_yaml_summary"].is_object(), "should have mind_yaml_summary");
}

#[test]
fn envelope_project_show_nonexistent_errors() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");

    let (stdout, stderr, code) =
        run_json(&["--root", &repo.path().to_string_lossy(), "project", "show", "nonexistent"]);
    assert_eq!(code, 2, "should error with usage exit code, stderr: {stderr:?}");
    if stdout.trim().is_empty() {
        assert!(!stderr.is_empty(), "stderr should have error message");
    } else {
        assert_envelope_err(&stdout);
        let err = extract_error(&stdout);
        assert!(err["message"].as_str().unwrap_or("").contains("not found"), "should mention 'not found': {stdout}");
    }
}

// ==========================================================================
// B1.4: mf project archive — envelope shape
// ==========================================================================

#[test]
fn envelope_project_archive_ok() {
    let dir = TempDir::new().unwrap();
    std::process::Command::new("git").args(["init", "-q"]).current_dir(dir.path()).output().unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    std::process::Command::new("git").args(["config", "user.name", "Test"]).current_dir(dir.path()).output().unwrap();
    std::fs::write(dir.path().join("minds.yaml"), "schema_version: '1'\nprojects_dir: '.'\nprojects: []\n").unwrap();
    let project_dir = dir.path().join("my-project");
    std::fs::create_dir_all(&project_dir).unwrap();
    std::fs::write(project_dir.join("mind.yaml"), "schema_version: '1'\n").unwrap();
    std::process::Command::new("git").args(["add", "-A"]).current_dir(dir.path()).output().unwrap();
    std::process::Command::new("git").args(["commit", "-m", "initial"]).current_dir(dir.path()).output().unwrap();

    let (stdout, stderr, code) =
        run_json(&["--root", &dir.path().to_string_lossy(), "project", "archive", "my-project", "--yes"]);
    assert_eq!(code, 0, "stderr: {stderr:?}");
    assert_envelope_ok(&stdout);

    let data = extract_data(&stdout);
    assert!(data["path"].is_string(), "path should be present");
    assert!(data["details"]["from"].is_string(), "details.from should be present");
    assert!(data["details"]["to"].is_string(), "details.to should be present");
}

#[test]
fn envelope_project_archive_non_git_error() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let (stdout, stderr, code) =
        run_json(&["--root", &repo.path().to_string_lossy(), "project", "archive", "my-project", "--yes"]);
    assert_eq!(code, 2, "should error outside git repo, stderr: {stderr:?}");
    // Error may go to stderr or stdout depending on dispatch path
    if stdout.trim().is_empty() {
        // Error was only on stderr — still valid for envelope shape
        assert!(!stderr.is_empty(), "stderr should have error message");
    } else {
        assert_envelope_err(&stdout);
    }
}

// ==========================================================================
// B1.5: mf project import — envelope shape
// ==========================================================================

#[test]
fn envelope_project_import_ok() {
    let repo = common::setup_repo();
    let import_dir = repo.path().join("to-import");
    std::fs::create_dir_all(import_dir.join("docs")).unwrap();
    std::fs::write(import_dir.join("docs/hello.md"), "# Hello").unwrap();

    let (stdout, stderr, code) =
        run_json(&["--root", &repo.path().to_string_lossy(), "project", "import", import_dir.to_str().unwrap(), "-y"]);
    assert_eq!(code, 0, "stderr: {stderr:?}");
    assert_envelope_ok(&stdout);

    let data = extract_data(&stdout);
    // Should have scaffolded=true or generated_mind_yaml path
    let has_result = data["scaffolded"].as_bool().is_some() || data["generated_mind_yaml"].is_string();
    assert!(has_result, "data should have result fields: {data}");
}

#[test]
fn envelope_project_import_nonexistent_dir_error() {
    let repo = common::setup_repo();

    let (stdout, stderr, code) =
        run_json(&["--root", &repo.path().to_string_lossy(), "project", "import", "/nonexistent/path", "-y"]);
    assert_eq!(code, 2, "stderr: {stderr:?}");
    if stdout.trim().is_empty() {
        assert!(!stderr.is_empty(), "stderr should have error message");
    } else {
        assert_envelope_err(&stdout);
    }
}

// ==========================================================================
// B2.1: mf config compile — envelope shape
// ==========================================================================

#[test]
fn envelope_config_compile_ok() {
    let dir = common::setup_repo();
    common::create_project(&dir, "my-project");

    // config compile removed → config show is the canonical replacement
    let (stdout, stderr, code) = run_json(&["--root", &dir.path().to_string_lossy(), "config", "show"]);
    assert_eq!(code, 0, "stderr: {stderr:?}");
    assert_envelope_ok(&stdout);
}

// ==========================================================================
// B2.2: mf config generate — envelope shape
// ==========================================================================

#[test]
fn envelope_config_generate_ok() {
    let dir = common::setup_repo();
    common::create_project(&dir, "my-project");
    let output_path = dir.path().join("mind.gen.yaml");

    let (stdout, stderr, code) = run_json(&[
        "--root",
        &dir.path().to_string_lossy(),
        "config",
        "generate",
        "--out",
        &output_path.to_string_lossy(),
    ]);
    assert_eq!(code, 0, "stderr: {stderr:?}");
    assert_envelope_ok(&stdout);

    let data = extract_data(&stdout);
    // May have path or generated_path
    assert!(data.is_object(), "data should be an object: {data}");
}

// ==========================================================================
// B2.3: mf config default — envelope shape
// ==========================================================================

#[test]
fn envelope_config_default_ok() {
    let (stdout, stderr, code) = run_json(&["config", "default"]);
    assert_eq!(code, 0, "stderr: {stderr:?}");
    assert_envelope_ok(&stdout);
}

// ==========================================================================
// B2.4: mf term show — envelope shape
// ==========================================================================

#[test]
fn envelope_term_show_ok() {
    let repo = common::setup_repo();
    setup_with_term(&repo);

    let (stdout, stderr, code) =
        run_json(&["--root", &repo.path().to_string_lossy(), "term", "show", "API", "--project", "alpha"]);
    assert_eq!(code, 0, "stderr: {stderr:?}");
    assert_envelope_ok(&stdout);

    let data = extract_data(&stdout);
    assert_eq!(data["term"], "API");
    assert!(data["definition"].is_string(), "definition should be a string");
    assert!(data["aliases"].is_array(), "aliases should be an array");
    assert!(data["tags"].is_array(), "tags should be an array");
}

#[test]
fn envelope_term_show_nonexistent_error() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");

    let (stdout, stderr, code) =
        run_json(&["--root", &repo.path().to_string_lossy(), "term", "show", "NonExistent", "--project", "alpha"]);
    assert_eq!(code, 2, "should error, stderr: {stderr:?}");
    if stdout.trim().is_empty() {
        assert!(!stderr.is_empty(), "stderr should have error message");
        assert!(stderr.contains("not found"), "stderr should mention 'not found': {stderr:?}");
    } else {
        assert_envelope_err(&stdout);
        let err = extract_error(&stdout);
        assert!(err["message"].as_str().unwrap_or("").contains("not found"), "should mention 'not found'");
    }
}

// ==========================================================================
// Deprecation: JSON envelope unchanged vs primary form
// ==========================================================================

#[test]
fn envelope_deprecated_term_list_term_json_matches_show() {
    let repo = common::setup_repo();
    setup_with_term(&repo);

    // --term flag removed from term list → use term show directly
    let (stdout, stderr, code) =
        run_json(&["--root", &repo.path().to_string_lossy(), "term", "show", "API", "--project", "alpha"]);
    assert_eq!(code, 0, "stderr: {stderr:?}");
    assert!(stderr.is_empty(), "term show should have clean stderr: {stderr:?}");
    assert_envelope_ok(&stdout);
}

// ==========================================================================
// Alias path: A-class alias JSON envelope matches primary
// ==========================================================================

#[test]
fn envelope_terms_alias_json_matches_term() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");

    let (stdout_terms, stderr_terms, code_terms) =
        run_json(&["--root", &repo.path().to_string_lossy(), "terms", "list", "--project", "alpha"]);
    assert_eq!(code_terms, 0, "stderr: {stderr_terms:?}");
    assert!(stderr_terms.is_empty(), "alias should not warn: {stderr_terms:?}");

    let (stdout_term, stderr_term, code_term) =
        run_json(&["--root", &repo.path().to_string_lossy(), "term", "list", "--project", "alpha"]);
    assert_eq!(code_term, 0, "stderr: {stderr_term:?}");

    assert_eq!(stdout_terms, stdout_term, "terms alias should match term output");
}

// ==========================================================================
// Render: JSON envelope shape
// ==========================================================================

#[test]
fn envelope_render_article_json() {
    let repo = common::setup_repo();
    common::create_project(&repo, "env-project");
    common::write_mind_yaml(
        &repo,
        "env-project",
        "schema_version: '1'\nbuild:\n  output_dir: 'outputs'\n  format: 'md'\n",
    );
    let project_dir = repo.path().join("env-project");
    std::fs::create_dir_all(project_dir.join("outputs")).unwrap();
    std::fs::write(project_dir.join("outputs/test-a.md"), b"# Envelope test").unwrap();

    let (stdout, stderr, code) =
        run_json(&["--root", &repo.path().to_string_lossy(), "render", "test-a", "--project", "env-project"]);
    assert_eq!(code, 0, "stderr: {stderr:?}");
    assert_envelope_ok(&stdout);

    let data = extract_data(&stdout);
    assert!(data["prompt"].is_string(), "prompt should be a string");
    assert_eq!(data["template"], "report");
    assert_eq!(data["template_source"], "built_in");
    assert_eq!(data["scope"], "article");
    assert!(data["outputs"].is_array(), "outputs should be an array");
}

#[test]
fn envelope_render_template_list_json() {
    let repo = common::setup_repo();

    let (stdout, stderr, code) = run_json(&["--root", &repo.path().to_string_lossy(), "render", "template", "list"]);
    assert_eq!(code, 0, "stderr: {stderr:?}");
    assert_envelope_ok(&stdout);

    let data = extract_data(&stdout);
    let templates = data["templates"].as_array().expect("templates should be an array");
    assert!(!templates.is_empty(), "should have at least one template");
    assert!(templates.iter().any(|t| t["name"] == "report"), "should include report");
    assert!(templates.iter().any(|t| t["name"] == "paper"), "should include paper");
}

#[test]
fn envelope_render_article_error_usage() {
    let repo = common::setup_repo();

    let (stdout, stderr, code) = run_json(&["--root", &repo.path().to_string_lossy(), "render", "nonexistent-article"]);
    assert_eq!(code, 2, "usage error should exit code 2, stderr: {stderr:?}");
    if stdout.trim().is_empty() {
        assert!(!stderr.is_empty(), "stderr should have error");
    } else {
        assert_envelope_err(&stdout);
    }
}

// ==========================================================================
// Article Convert: JSON envelope shape
// ==========================================================================

#[test]
fn envelope_article_convert_to_single_file_json() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    // Create a directory article
    let article_dir = repo.path().join("my-project/docs/my-article");
    std::fs::create_dir_all(&article_dir).unwrap();
    std::fs::write(article_dir.join("01-opening.md"), b"# My Article\n\nContent.\n").unwrap();

    let index_yaml = r#"schema_version: '1'
articles:
  - title: 'My Article'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/my-article'
    status: draft
    created_at: '2026-05-15T00:00:00Z'
    updated_at: '2026-05-15T00:00:00Z'
"#;
    common::write_index(&repo, "my-project", index_yaml);

    let (stdout, stderr, code) = run_json(&[
        "--root",
        &repo.path().to_string_lossy(),
        "article",
        "convert",
        "--to-single-file",
        "--project",
        "my-project",
    ]);
    assert_eq!(code, 0, "stderr: {stderr:?} stdout: {stdout}");
    assert_envelope_ok(&stdout);

    let data = extract_data(&stdout);
    assert_eq!(data["kind"], "article");
    assert_eq!(data["direction"], "to_single_file");
    assert_eq!(data["direction_source"], "explicit");
    assert_eq!(data["dry_run"], false);
    assert_eq!(data["converted_count"], 1);
    assert_eq!(data["skipped_count"], 0);
    assert_eq!(data["failed_count"], 0);

    let converted = data["converted"].as_array().unwrap();
    assert_eq!(converted.len(), 1);
    assert_eq!(converted[0]["status"], "converted");
    assert_eq!(converted[0]["direction"], "to_single_file");
    assert_eq!(converted[0]["source_shape"], "directory");
    assert_eq!(converted[0]["target_shape"], "single_file");
    assert_eq!(converted[0]["source_path"], "docs/my-article");
    assert_eq!(converted[0]["target_path"], "docs/my-article.md");
    assert_eq!(converted[0]["index_updated"], true);
    assert_eq!(converted[0]["source_removed"], true);
}

#[test]
fn envelope_article_convert_to_directory_json() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    // Create a single-file article
    std::fs::create_dir_all(repo.path().join("my-project/docs")).unwrap();
    std::fs::write(repo.path().join("my-project/docs/my-article.md"), b"# My Article\n\nContent.\n").unwrap();

    let index_yaml = r#"schema_version: '1'
articles:
  - title: 'My Article'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/my-article.md'
    status: draft
    created_at: '2026-05-15T00:00:00Z'
    updated_at: '2026-05-15T00:00:00Z'
"#;
    common::write_index(&repo, "my-project", index_yaml);

    let (stdout, stderr, code) = run_json(&[
        "--root",
        &repo.path().to_string_lossy(),
        "article",
        "convert",
        "--to-directory",
        "--project",
        "my-project",
    ]);
    assert_eq!(code, 0, "stderr: {stderr:?} stdout: {stdout}");
    assert_envelope_ok(&stdout);

    let data = extract_data(&stdout);
    assert_eq!(data["kind"], "article");
    assert_eq!(data["direction"], "to_directory");
    assert_eq!(data["direction_source"], "explicit");
    assert_eq!(data["dry_run"], false);
    assert_eq!(data["converted_count"], 1);
    assert_eq!(data["skipped_count"], 0);
    assert_eq!(data["failed_count"], 0);

    let converted = data["converted"].as_array().unwrap();
    assert_eq!(converted.len(), 1);
    assert_eq!(converted[0]["status"], "converted");
    assert_eq!(converted[0]["direction"], "to_directory");
    assert_eq!(converted[0]["source_shape"], "single_file");
    assert_eq!(converted[0]["target_shape"], "directory");
    assert_eq!(converted[0]["source_path"], "docs/my-article.md");
    assert_eq!(converted[0]["target_path"], "docs/my-article");
    assert_eq!(converted[0]["target_content_path"], "docs/my-article/01-opening.md");
    assert_eq!(converted[0]["index_updated"], true);
    assert_eq!(converted[0]["source_removed"], true);
}

#[test]
fn envelope_article_convert_dry_run_json() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let article_dir = repo.path().join("my-project/docs/my-article");
    std::fs::create_dir_all(&article_dir).unwrap();
    std::fs::write(article_dir.join("01-opening.md"), b"# My Article\n\nContent.\n").unwrap();

    let index_yaml = r#"schema_version: '1'
articles:
  - title: 'My Article'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/my-article'
    status: draft
    created_at: '2026-05-15T00:00:00Z'
    updated_at: '2026-05-15T00:00:00Z'
"#;
    common::write_index(&repo, "my-project", index_yaml);

    let (stdout, stderr, code) = run_json(&[
        "--root",
        &repo.path().to_string_lossy(),
        "article",
        "convert",
        "--to-single-file",
        "--dry-run",
        "--project",
        "my-project",
    ]);
    assert_eq!(code, 0, "stderr: {stderr:?} stdout: {stdout}");
    assert_envelope_ok(&stdout);

    let data = extract_data(&stdout);
    assert_eq!(data["kind"], "article");
    assert_eq!(data["dry_run"], true);
    assert_eq!(data["converted_count"], 1);
    assert_eq!(data["skipped_count"], 0);

    let converted = data["converted"].as_array().unwrap();
    assert_eq!(converted.len(), 1);
    assert_eq!(converted[0]["status"], "would_convert");
    assert_eq!(converted[0]["index_updated"], false);
    assert_eq!(converted[0]["source_removed"], false);
}

// ── T056: JSON skip-reason assertions ─────────────────

#[test]
fn envelope_article_convert_skip_no_section_files_json() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    // Empty directory article (no section files)
    let article_dir = repo.path().join("my-project/docs/empty-article");
    std::fs::create_dir_all(&article_dir).unwrap();

    let index_yaml = r#"schema_version: '1'
articles:
  - title: 'Empty Article'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/empty-article'
    status: draft
    created_at: '2026-05-15T00:00:00Z'
    updated_at: '2026-05-15T00:00:00Z'
"#;
    common::write_index(&repo, "my-project", index_yaml);

    let (stdout, stderr, code) = run_json(&[
        "--root",
        &repo.path().to_string_lossy(),
        "article",
        "convert",
        "--to-single-file",
        "--project",
        "my-project",
    ]);
    assert_eq!(code, 0, "stderr: {stderr:?} stdout: {stdout}");
    assert_envelope_ok(&stdout);

    let data = extract_data(&stdout);
    assert_eq!(data["converted_count"], 0);
    assert_eq!(data["skipped_count"], 1);
    assert_eq!(data["failed_count"], 0);

    let skipped = data["skipped"].as_array().unwrap();
    assert_eq!(skipped.len(), 1);
    assert_eq!(skipped[0]["status"], "skipped");
    assert_eq!(skipped[0]["reason"], "no_section_files");
    assert_eq!(skipped[0]["source_shape"], "directory");
    assert_eq!(skipped[0]["source_path"], "docs/empty-article");
}

#[test]
fn envelope_article_convert_skip_multiple_section_files_json() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    // Directory article with 2 section files
    let article_dir = repo.path().join("my-project/docs/multi-article");
    std::fs::create_dir_all(&article_dir).unwrap();
    std::fs::write(article_dir.join("01-opening.md"), b"# Opening\n").unwrap();
    std::fs::write(article_dir.join("02-body.md"), b"# Body\n").unwrap();

    let index_yaml = r#"schema_version: '1'
articles:
  - title: 'Multi Article'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/multi-article'
    status: draft
    created_at: '2026-05-15T00:00:00Z'
    updated_at: '2026-05-15T00:00:00Z'
"#;
    common::write_index(&repo, "my-project", index_yaml);

    let (stdout, stderr, code) = run_json(&[
        "--root",
        &repo.path().to_string_lossy(),
        "article",
        "convert",
        "--to-single-file",
        "--project",
        "my-project",
    ]);
    assert_eq!(code, 0, "stderr: {stderr:?} stdout: {stdout}");
    assert_envelope_ok(&stdout);

    let data = extract_data(&stdout);
    assert_eq!(data["converted_count"], 0);
    assert_eq!(data["skipped_count"], 1);

    let skipped = data["skipped"].as_array().unwrap();
    assert_eq!(skipped.len(), 1);
    assert_eq!(skipped[0]["status"], "skipped");
    assert_eq!(skipped[0]["reason"], "multiple_section_files");
}

#[test]
fn envelope_article_convert_skip_target_exists_json() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    // Directory article + pre-existing target .md file
    let article_dir = repo.path().join("my-project/docs/my-article");
    std::fs::create_dir_all(&article_dir).unwrap();
    std::fs::write(article_dir.join("01-opening.md"), b"# My Article\n\nContent.\n").unwrap();
    // Target file already exists
    std::fs::write(repo.path().join("my-project/docs/my-article.md"), b"existing content\n").unwrap();

    let index_yaml = r#"schema_version: '1'
articles:
  - title: 'My Article'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/my-article'
    status: draft
    created_at: '2026-05-15T00:00:00Z'
    updated_at: '2026-05-15T00:00:00Z'
"#;
    common::write_index(&repo, "my-project", index_yaml);

    let (stdout, stderr, code) = run_json(&[
        "--root",
        &repo.path().to_string_lossy(),
        "article",
        "convert",
        "--to-single-file",
        "--project",
        "my-project",
    ]);
    assert_eq!(code, 0, "stderr: {stderr:?} stdout: {stdout}");
    assert_envelope_ok(&stdout);

    let data = extract_data(&stdout);
    assert_eq!(data["converted_count"], 0);
    assert_eq!(data["skipped_count"], 1);

    let skipped = data["skipped"].as_array().unwrap();
    assert_eq!(skipped.len(), 1);
    assert_eq!(skipped[0]["status"], "skipped");
    assert_eq!(skipped[0]["reason"], "target_exists");
}

#[test]
fn envelope_article_convert_skip_extra_files_json() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    // Directory article with extra non-section files
    let article_dir = repo.path().join("my-project/docs/my-article");
    std::fs::create_dir_all(&article_dir).unwrap();
    std::fs::write(article_dir.join("01-opening.md"), b"# My Article\n\nContent.\n").unwrap();
    std::fs::write(article_dir.join("notes.txt"), b"some notes\n").unwrap();

    let index_yaml = r#"schema_version: '1'
articles:
  - title: 'My Article'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/my-article'
    status: draft
    created_at: '2026-05-15T00:00:00Z'
    updated_at: '2026-05-15T00:00:00Z'
"#;
    common::write_index(&repo, "my-project", index_yaml);

    let (stdout, stderr, code) = run_json(&[
        "--root",
        &repo.path().to_string_lossy(),
        "article",
        "convert",
        "--to-single-file",
        "--project",
        "my-project",
    ]);
    assert_eq!(code, 0, "stderr: {stderr:?} stdout: {stdout}");
    assert_envelope_ok(&stdout);

    let data = extract_data(&stdout);
    assert_eq!(data["converted_count"], 0);
    assert_eq!(data["skipped_count"], 1);

    let skipped = data["skipped"].as_array().unwrap();
    assert_eq!(skipped.len(), 1);
    assert_eq!(skipped[0]["status"], "skipped");
    assert_eq!(skipped[0]["reason"], "extra_files");
}

#[test]
fn envelope_article_convert_skip_not_directory_article_json() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    // Single-file article, but requesting to-single-file (already in target shape)
    std::fs::create_dir_all(repo.path().join("my-project/docs")).unwrap();
    std::fs::write(repo.path().join("my-project/docs/my-article.md"), b"# My Article\n\nContent.\n").unwrap();

    let index_yaml = r#"schema_version: '1'
articles:
  - title: 'My Article'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/my-article.md'
    status: draft
    created_at: '2026-05-15T00:00:00Z'
    updated_at: '2026-05-15T00:00:00Z'
"#;
    common::write_index(&repo, "my-project", index_yaml);

    let (stdout, stderr, code) = run_json(&[
        "--root",
        &repo.path().to_string_lossy(),
        "article",
        "convert",
        "--to-single-file",
        "--project",
        "my-project",
    ]);
    assert_eq!(code, 0, "stderr: {stderr:?} stdout: {stdout}");
    assert_envelope_ok(&stdout);

    let data = extract_data(&stdout);
    assert_eq!(data["converted_count"], 0);
    assert_eq!(data["skipped_count"], 1);

    let skipped = data["skipped"].as_array().unwrap();
    assert_eq!(skipped.len(), 1);
    assert_eq!(skipped[0]["status"], "skipped");
    assert_eq!(skipped[0]["reason"], "not_directory_article");
}

#[test]
fn envelope_article_convert_skip_not_single_file_article_json() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    // Directory article, but requesting to-directory (already in target shape)
    let article_dir = repo.path().join("my-project/docs/my-article");
    std::fs::create_dir_all(&article_dir).unwrap();
    std::fs::write(article_dir.join("01-opening.md"), b"# My Article\n\nContent.\n").unwrap();

    let index_yaml = r#"schema_version: '1'
articles:
  - title: 'My Article'
    project: 'my-project'
    article_type: blog
    article_path: 'docs/my-article'
    status: draft
    created_at: '2026-05-15T00:00:00Z'
    updated_at: '2026-05-15T00:00:00Z'
"#;
    common::write_index(&repo, "my-project", index_yaml);

    let (stdout, stderr, code) = run_json(&[
        "--root",
        &repo.path().to_string_lossy(),
        "article",
        "convert",
        "--to-directory",
        "--project",
        "my-project",
    ]);
    assert_eq!(code, 0, "stderr: {stderr:?} stdout: {stdout}");
    assert_envelope_ok(&stdout);

    let data = extract_data(&stdout);
    assert_eq!(data["converted_count"], 0);
    assert_eq!(data["skipped_count"], 1);

    let skipped = data["skipped"].as_array().unwrap();
    assert_eq!(skipped.len(), 1);
    assert_eq!(skipped[0]["status"], "skipped");
    assert_eq!(skipped[0]["reason"], "not_single_file_article");
}
