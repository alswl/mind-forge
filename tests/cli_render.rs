use assert_cmd::Command;
use std::fs;
use std::path::PathBuf;

mod common;

// =============================================================================
// Helpers
// =============================================================================

fn run(args: &[&str]) -> (String, String, i32) {
    let output = Command::cargo_bin("mf").expect("binary exists").args(args).output().expect("command runs");
    (
        String::from_utf8(output.stdout).expect("stdout utf8"),
        String::from_utf8(output.stderr).expect("stderr utf8"),
        output.status.code().unwrap_or_default(),
    )
}

fn run_json(args: &[&str]) -> (String, String, i32) {
    let mut full_args = vec!["--format", "json"];
    full_args.extend_from_slice(args);
    run(&full_args)
}

const BUILD_CONFIG: &str = "schema_version: '1'\nbuild:\n  output_dir: 'outputs'\n  format: 'md'\n";

fn setup_project(repo: &common::TempDir, name: &str) -> PathBuf {
    common::create_project(repo, name);
    common::write_mind_yaml(repo, name, BUILD_CONFIG);
    repo.path().join(name)
}

/// Set up a minimal repo with a project that has a built output artifact.
fn setup_with_article_output() -> (common::TempDir, String) {
    let repo = common::setup_repo();
    let project_dir = setup_project(&repo, "test-project");
    let output_dir = project_dir.join("outputs");
    fs::create_dir_all(&output_dir).unwrap();
    fs::write(output_dir.join("test-article.md"), "# Test Content\n\nHello world.\n").unwrap();
    let root = repo.path().to_string_lossy().to_string();
    (repo, root)
}

fn setup_with_empty_output() -> (common::TempDir, String) {
    let repo = common::setup_repo();
    let project_dir = setup_project(&repo, "test-project");
    let output_dir = project_dir.join("outputs");
    fs::create_dir_all(&output_dir).unwrap();
    fs::write(output_dir.join("empty-article.md"), "").unwrap();
    let root = repo.path().to_string_lossy().to_string();
    (repo, root)
}

// =============================================================================
// US1: Article render prompt generation
// =============================================================================

#[test]
fn test_render_article_default_prompt_success() {
    let (_repo, root) = setup_with_article_output();

    let (stdout, stderr, code) = run(&["--root", &root, "render", "test-article", "--project", "test-project"]);

    assert_eq!(code, 0, "expected success, stderr: {stderr:?}");
    assert!(stderr.is_empty(), "stderr should be empty on success: {stderr:?}");

    // stdout should be the prompt (with trailing newline)
    assert!(!stdout.is_empty(), "stdout should contain the prompt");
    assert!(stdout.contains("You are an Agent"), "prompt should identify the Agent role");
    assert!(stdout.contains("HTML"), "prompt should mention HTML");
    assert!(stdout.contains("report"), "default template should be 'report'");
    assert!(stdout.contains("mf is NOT rendering"), "prompt should clarify mf does not render");
    assert!(stdout.contains("BEGIN MF OUTPUT: outputs/test-article.md"), "prompt should include output boundary");
    assert!(stdout.contains("END MF OUTPUT: outputs/test-article.md"), "prompt should include closing boundary");
    assert!(stdout.contains("# Test Content"), "prompt should include article content");
}

#[test]
fn test_render_article_no_html_file_created() {
    let (_repo, root) = setup_with_article_output();

    let (stdout, stderr, code) = run(&["--root", &root, "render", "test-article", "--project", "test-project"]);

    assert_eq!(code, 0, "stderr: {stderr:?}");
    assert!(stderr.is_empty(), "stderr should be empty: {stderr:?}");
    assert!(!stdout.is_empty(), "stdout should have prompt output");

    // Verify no HTML file was created in the project
    let html_files: Vec<_> = walkdir_files(&_repo.path().join("test-project"), ".html");
    assert!(html_files.is_empty(), "render should not create any HTML files: {html_files:?}");
}

#[test]
fn test_render_article_json_envelope() {
    let (_repo, root) = setup_with_article_output();

    let (stdout, stderr, code) = run_json(&["--root", &root, "render", "test-article", "--project", "test-project"]);

    assert_eq!(code, 0, "stderr: {stderr:?}");
    assert!(stderr.is_empty(), "stderr should be empty: {stderr:?}");

    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert_eq!(v["status"], "ok", "envelope status should be ok");
    assert_eq!(v["command"], "mf", "envelope command should be mf");

    let data = &v["data"];
    assert!(data["prompt"].is_string(), "data.prompt should be a string");
    assert!(data["prompt"].as_str().unwrap().contains("HTML"), "prompt should mention HTML");
    assert_eq!(data["template"], "report", "default template should be report");
    assert_eq!(data["template_source"], "built_in", "template source should be built_in");
    assert_eq!(data["scope"], "article", "scope should be article");
    assert_eq!(data["project"], "test-project", "project should match");
    assert_eq!(data["article"], "test-article", "article should match");

    let outputs = data["outputs"].as_array().expect("outputs should be an array");
    assert_eq!(outputs.len(), 1, "should have 1 output");
    assert_eq!(outputs[0]["path"], "outputs/test-article.md");
    assert!(outputs[0]["size_bytes"].as_u64().unwrap_or(0) > 0, "size should be > 0");
}

#[test]
fn test_render_article_missing_output_error() {
    let repo = common::setup_repo();
    common::create_project(&repo, "test-project");
    common::write_mind_yaml(
        &repo,
        "test-project",
        "schema_version: '1'\nbuild:\n  output_dir: 'outputs'\n  format: 'md'\n",
    );
    let root = repo.path().to_string_lossy().to_string();

    let (stdout, stderr, code) = run(&["--root", &root, "render", "nonexistent-article", "--project", "test-project"]);

    assert_eq!(code, 1, "should exit with code 1 for not found");
    assert!(
        stdout.contains("not found") || stderr.contains("not found") || stderr.contains("output not found"),
        "error should mention 'not found': stderr={stderr:?} stdout={stdout:?}"
    );
}

#[test]
fn test_render_article_empty_output_error() {
    let (_repo, root) = setup_with_empty_output();

    let (stdout, stderr, code) = run(&["--root", &root, "render", "empty-article", "--project", "test-project"]);

    assert_eq!(code, 1, "should exit with code 1 for empty output");
    assert!(
        stdout.contains("empty") || stderr.contains("empty"),
        "error should mention 'empty': stderr={stderr:?} stdout={stdout:?}"
    );
}

#[test]
fn test_render_article_missing_project_error() {
    let (_repo, root) = setup_with_article_output();

    // Run without a project context (outside project dir and no --project)
    let (stdout, stderr, code) = run(&["--root", &root, "render", "test-article"]);

    assert_eq!(code, 2, "should exit with code 2 for usage error");
    assert!(
        stderr.contains("detect current project") || stdout.contains("detect current project"),
        "error should mention project detection: stderr={stderr:?}"
    );
}

// Simple walker to check for HTML files
fn walkdir_files(dir: &std::path::Path, ext: &str) -> Vec<String> {
    let mut results = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                results.extend(walkdir_files(&path, ext));
            } else if path.extension().map(|e| e == ext.trim_start_matches('.')).unwrap_or(false) {
                results.push(path.to_string_lossy().to_string());
            }
        }
    }
    results
}

// =============================================================================
// US2: Template selection and listing
// =============================================================================

#[test]
fn test_render_template_selection_changes_prompt() {
    let (_repo, root) = setup_with_article_output();

    let (stdout_report, _, _) =
        run(&["--root", &root, "render", "test-article", "--project", "test-project", "--template", "report"]);
    let (stdout_paper, _, _) =
        run(&["--root", &root, "render", "test-article", "--project", "test-project", "--template", "paper"]);

    // Both should succeed with different guidance
    assert!(stdout_report.contains("executive summary"), "report should mention executive summary");
    assert!(stdout_paper.contains("abstract"), "paper should mention abstract");
    assert_ne!(stdout_report, stdout_paper, "report and paper should produce different prompts");
}

#[test]
fn test_render_template_list_text() {
    let (_repo, root) = setup_with_article_output();

    let (stdout, stderr, code) = run(&["--root", &root, "render", "template", "list"]);

    assert_eq!(code, 0, "stderr: {stderr:?}");
    assert!(stderr.is_empty(), "stderr should be empty: {stderr:?}");
    assert!(stdout.contains("report"), "should list 'report'");
    assert!(stdout.contains("paper"), "should list 'paper'");
    assert!(stdout.contains("Work report"), "report label");
    assert!(stdout.contains("Paper"), "paper label");
}

#[test]
fn test_render_template_list_json() {
    let (_repo, root) = setup_with_article_output();

    let (stdout, stderr, code) = run_json(&["--root", &root, "render", "template", "list"]);

    assert_eq!(code, 0, "stderr: {stderr:?}");
    assert!(stderr.is_empty(), "stderr should be empty: {stderr:?}");

    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(v["status"], "ok", "status should be ok");

    let templates = v["data"]["templates"].as_array().expect("templates should be array");
    assert!(templates.len() >= 2, "should have at least 2 templates");

    let report = templates.iter().find(|t| t["name"] == "report").expect("should have report");
    assert_eq!(report["source"], "built_in");
    assert_eq!(report["default"], true);

    let paper = templates.iter().find(|t| t["name"] == "paper").expect("should have paper");
    assert_eq!(paper["source"], "built_in");
    assert_eq!(paper["default"], false);
}

#[test]
fn test_render_custom_template_discovery() {
    let (_repo, root) = setup_with_article_output();

    // Write a custom template
    let templates_dir = std::path::Path::new(&root).join(".mind-forge").join("renders");
    fs::create_dir_all(&templates_dir).unwrap();
    fs::write(
        templates_dir.join("team-review.md"),
        "---\nlabel: Team Review\ndescription: Internal review page for leadership\n---\n\nRender as a review page for leadership.",
    ).unwrap();

    // It should appear in template list
    let (stdout, stderr, code) = run(&["--root", &root, "render", "template", "list"]);
    assert_eq!(code, 0, "stderr: {stderr:?}");
    assert!(stdout.contains("team-review"), "custom template should appear in list");
    assert!(stdout.contains("Team Review"), "custom template label should appear");
    assert!(stdout.contains("custom"), "custom source indicator should appear");

    // It should work as --template
    let (stdout2, stderr2, code2) =
        run(&["--root", &root, "render", "test-article", "--project", "test-project", "--template", "team-review"]);
    assert_eq!(code2, 0, "custom template render should succeed, stderr: {stderr2:?}");
    assert!(stdout2.contains("review page for leadership"), "custom template guidance should appear");
    assert!(stdout2.contains("mf is NOT rendering"), "standard boilerplate should still appear");
}

#[test]
fn test_render_unknown_template_error() {
    let (_repo, root) = setup_with_article_output();

    let (stdout, stderr, code) = run(&[
        "--root",
        &root,
        "render",
        "test-article",
        "--project",
        "test-project",
        "--template",
        "nonexistent-template",
    ]);

    assert_eq!(code, 2, "should exit with code 2 for usage error");
    assert!(
        stdout.contains("unknown template") || stderr.contains("unknown template"),
        "error should mention 'unknown template': stderr={stderr:?}"
    );
}

#[test]
fn test_render_builtin_name_conflict_error() {
    let (_repo, root) = setup_with_article_output();

    // Write a custom template with the same name as a built-in
    let templates_dir = std::path::Path::new(&root).join(".mind-forge").join("renders");
    fs::create_dir_all(&templates_dir).unwrap();
    fs::write(templates_dir.join("report.md"), "# This conflicts with built-in report").unwrap();

    // Running list should fail with conflict error
    let (stdout, stderr, code) = run(&["--root", &root, "render", "template", "list"]);

    assert_eq!(code, 2, "should exit with code 2 for conflict");
    assert!(
        stdout.contains("conflict") || stderr.contains("conflict"),
        "error should mention 'conflict': stderr={stderr:?}"
    );
}

#[test]
fn test_render_empty_custom_template_error_identifies_file() {
    let (_repo, root) = setup_with_article_output();

    let templates_dir = std::path::Path::new(&root).join(".mind-forge").join("renders");
    fs::create_dir_all(&templates_dir).unwrap();
    fs::write(templates_dir.join("team-review.md"), "   \n").unwrap();

    let (stdout, stderr, code) = run(&["--root", &root, "render", "template", "list"]);

    assert_eq!(code, 2, "should exit with code 2 for empty custom template");
    assert!(
        stdout.contains(".mind-forge/renders/team-review.md") || stderr.contains(".mind-forge/renders/team-review.md"),
        "error should identify empty template file: stderr={stderr:?}"
    );
}

#[test]
fn test_render_invalid_template_name_error() {
    let (_repo, root) = setup_with_article_output();

    let (stdout, stderr, code) = run(&[
        "--root",
        &root,
        "render",
        "test-article",
        "--project",
        "test-project",
        "--template",
        "path/with/slashes",
    ]);

    assert_eq!(code, 2, "should exit with code 2 for usage error");
    assert!(
        stdout.contains("path separators") || stderr.contains("path separators") || stderr.contains("invalid template"),
        "error should mention path separator issue: stderr={stderr:?}"
    );
}

// =============================================================================
// US3: Project scope and HTML form controls
// =============================================================================

fn setup_with_multiple_outputs() -> (common::TempDir, String) {
    let repo = common::setup_repo();
    let project_dir = setup_project(&repo, "test-project");
    let output_dir = project_dir.join("outputs");
    fs::create_dir_all(&output_dir).unwrap();
    fs::write(output_dir.join("article-one.md"), "# Article One\n\nFirst article.\n").unwrap();
    fs::write(output_dir.join("article-two.md"), "# Article Two\n\nSecond article.\n").unwrap();
    let root = repo.path().to_string_lossy().to_string();
    (repo, root)
}

#[test]
fn test_render_project_scope_gathers_outputs() {
    let (_repo, root) = setup_with_multiple_outputs();

    // Project scope: omit ARTICLE, use --project
    let (stdout, stderr, code) = run(&["--root", &root, "render", "--project", "test-project"]);

    assert_eq!(code, 0, "project-scope render should succeed, stderr: {stderr:?}");
    assert!(stderr.is_empty(), "stderr should be empty: {stderr:?}");

    // Should include both articles
    assert!(stdout.contains("article-one"), "should include article-one output");
    assert!(stdout.contains("article-two"), "should include article-two output");
    assert!(stdout.contains("BEGIN MF OUTPUT"), "should contain output boundaries");
}

#[test]
fn test_render_project_scope_json() {
    let (_repo, root) = setup_with_multiple_outputs();

    let (stdout, stderr, code) = run_json(&["--root", &root, "render", "--project", "test-project"]);

    assert_eq!(code, 0, "stderr: {stderr:?}");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(v["status"], "ok");
    assert_eq!(v["data"]["scope"], "project", "scope should be project");
    assert!(v["data"]["article"].is_null(), "article should be null for project scope");

    let outputs = v["data"]["outputs"].as_array().expect("outputs as array");
    assert_eq!(outputs.len(), 2, "should have 2 outputs");
}

#[test]
fn test_render_html_form_document_prompt() {
    let (_repo, root) = setup_with_article_output();

    let (stdout, _, code) =
        run(&["--root", &root, "render", "test-article", "--project", "test-project", "--html-form", "document"]);

    assert_eq!(code, 0, "should succeed");
    assert!(stdout.contains("complete HTML document"), "should mention 'complete HTML document'");
    assert!(stdout.contains("<html>"), "should mention <html> tag");
}

#[test]
fn test_render_html_form_fragment_prompt() {
    let (_repo, root) = setup_with_article_output();

    let (stdout, _, code) =
        run(&["--root", &root, "render", "test-article", "--project", "test-project", "--html-form", "fragment"]);

    assert_eq!(code, 0, "should succeed");
    assert!(stdout.contains("HTML fragment"), "should mention 'HTML fragment'");
    assert!(stdout.contains("no <html>"), "should mention exclusion of outer tags");
}

#[test]
fn test_render_project_no_outputs_error() {
    let repo = common::setup_repo();
    setup_project(&repo, "empty-project");
    let root = repo.path().to_string_lossy().to_string();

    // Project scope with no outputs at all
    let (stdout, stderr, code) = run(&["--root", &root, "render", "--project", "empty-project"]);

    assert_eq!(code, 1, "should exit with code 1 for not found");
    assert!(
        stdout.contains("no renderable output")
            || stderr.contains("no renderable output")
            || stderr.contains("output directory"),
        "error should mention no renderable output: stderr={stderr:?}"
    );
}
