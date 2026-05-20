use std::fs;

use assert_cmd::Command;

mod common;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn mf() -> Command {
    Command::cargo_bin("mf").unwrap()
}

// ============================================================================
// US1: Layout visibility — config default, config show, project show
// ============================================================================

/// T015: `mf config default --output-format yaml` includes full default layout.
#[test]
fn config_default_yaml_includes_layout_block() {
    let output = mf().args(["config", "default", "--output-format", "yaml"]).output().expect("command runs");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("layout:"), "default yaml should contain layout block:\n{stdout}");
    assert!(stdout.contains("articles:"), "layout should have articles");
    assert!(stdout.contains("sources:"), "layout should have sources");
    assert!(stdout.contains("assets:"), "layout should have assets");
    assert!(stdout.contains("templates:"), "layout should have templates");
    assert!(stdout.contains("build_output:"), "layout should have build_output");
}

/// T016: `mf config default --output-format json` includes all layout fields.
#[test]
fn config_default_json_includes_layout_fields() {
    let output = mf().args(["config", "default", "--output-format", "json"]).output().expect("command runs");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let layout = &parsed["layout"];
    assert_eq!(layout["articles"], "docs");
    assert_eq!(layout["sources"], "sources");
    assert_eq!(layout["assets"], "assets");
    assert_eq!(layout["templates"], "templates");
    assert_eq!(layout["build_output"], "outputs");
}

/// T017: `mf config show --output-format json` merges partial layout with defaults.
#[test]
fn config_show_merges_partial_layout_with_defaults() {
    let repo = common::setup_repo();
    common::write_mind_yaml(&repo, "blog", "schema: '1'\nlayout:\n  articles: entries\n  assets: media\n");

    let output = mf()
        .current_dir(repo.path().join("blog"))
        .args(["config", "show", "--output-format", "json"])
        .output()
        .expect("command runs");
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let layout = &parsed["layout"];
    assert_eq!(layout["articles"], "entries");
    assert_eq!(layout["sources"], "sources"); // default
    assert_eq!(layout["assets"], "media");
    assert_eq!(layout["templates"], "templates"); // default
    assert_eq!(layout["build_output"], "outputs"); // default
}

/// T018: `mf project show` JSON exposes effective layout.
#[test]
fn project_show_json_includes_effective_layout() {
    let repo = common::setup_repo();
    common::write_mind_yaml(&repo, "blog", "schema: '1'\nlayout:\n  articles: entries\n  build_output: dist\n");

    let output = mf()
        .current_dir(repo.path())
        .args(["--format", "json", "project", "show", "blog"])
        .output()
        .expect("command runs");
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json_start = stdout.find('{').unwrap_or(0);
    let parsed: serde_json::Value = serde_json::from_str(&stdout[json_start..]).unwrap();

    assert_eq!(parsed["status"], "ok");
    let layout = &parsed["data"]["layout"];
    assert_eq!(layout["articles"], "entries");
    assert_eq!(layout["build_output"], "dist");
    assert_eq!(layout["sources"], "sources"); // default
}

// ============================================================================
// US2: Rename directories — commands use effective layout paths
// ============================================================================

/// T025: `layout.articles: articles` makes `mf article new` create files under articles/.
#[test]
fn article_new_uses_configured_articles_dir() {
    let repo = common::setup_repo();
    common::write_mind_yaml(&repo, "proj", "schema: '1'\nlayout:\n  articles: articles\n");

    mf().current_dir(repo.path().join("proj")).args(["article", "new", "First Post"]).assert().success();

    assert!(
        repo.path().join("proj/articles/first-post/01-opening.md").exists(),
        "article should be created under configured articles/ dir"
    );
    assert!(
        !repo.path().join("proj/docs").exists(),
        "default docs/ dir should not be created when layout.articles is renamed"
    );
}

/// T026: `layout.sources: references` appears in effective layout via config show.
#[test]
fn source_dir_configured_in_effective_layout() {
    let repo = common::setup_repo();
    common::write_mind_yaml(&repo, "proj", "schema: '1'\nlayout:\n  sources: references\n");

    // The effective layout should show the configured sources dir
    let output = mf()
        .current_dir(repo.path().join("proj"))
        .args(["config", "show", "--output-format", "json"])
        .output()
        .expect("command runs");
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed["layout"]["sources"], "references");
}

/// T027: `layout.assets: media` makes asset add/list/index use media/.
#[test]
fn asset_add_uses_configured_assets_dir() {
    let repo = common::setup_repo();
    common::write_mind_yaml(&repo, "proj", "schema: '1'\nlayout:\n  assets: media\n");

    let src_dir = tempfile::TempDir::new().unwrap();
    let src_file = src_dir.path().join("logo.png");
    fs::write(&src_file, b"fake png").unwrap();

    mf().current_dir(repo.path().join("proj"))
        .args(["--root", repo.path().to_str().unwrap(), "asset", "add", src_file.to_str().unwrap()])
        .assert()
        .success();

    assert!(repo.path().join("proj/media/logo.png").exists(), "asset should be added under configured media/ dir");
}

/// T028: `layout.templates: guides` — article creation still works with custom templates dir.
#[test]
fn article_new_uses_configured_templates_dir() {
    let repo = common::setup_repo();
    common::write_mind_yaml(&repo, "proj", "schema: '1'\nlayout:\n  templates: guides\n");

    // Create a template in the configured templates dir
    fs::create_dir_all(repo.path().join("proj/guides")).unwrap();
    fs::write(repo.path().join("proj/guides/default-template.md"), b"# Template content\n").unwrap();

    // Article new should still work; templates dir is for creation guide lookup
    mf().current_dir(repo.path().join("proj")).args(["article", "new", "Template Test"]).assert().success();

    assert!(
        repo.path().join("proj/docs/template-test/01-opening.md").exists(),
        "article should be created even with custom templates dir"
    );
}

/// T029: `layout.build_output: dist` makes `mf build` write under dist/.
#[test]
fn build_uses_configured_build_output_dir() {
    let repo = common::setup_repo();
    common::write_mind_yaml(&repo, "proj", "schema: '1'\nlayout:\n  build_output: dist\n");

    // Create a directory article
    mf().current_dir(repo.path().join("proj")).args(["article", "new", "Build Test"]).assert().success();

    // Build should use the configured output dir
    mf().current_dir(repo.path().join("proj")).args(["build", "build-test"]).assert().success();

    assert!(repo.path().join("proj/dist/build-test.md").exists(), "build output should be under configured dist/ dir");
}

/// T030: Read-only commands do not create missing directories.
#[test]
fn read_only_commands_do_not_create_missing_dirs() {
    let repo = common::setup_repo();
    common::write_mind_yaml(
        &repo,
        "proj",
        "schema: '1'\nlayout:\n  articles: entries\n  sources: refs\n  assets: media\n",
    );

    // Run a read-only command (config show)
    mf().current_dir(repo.path().join("proj")).args(["config", "show", "--output-format", "json"]).assert().success();

    // Default directories should NOT have been created
    assert!(!repo.path().join("proj/docs").exists(), "docs/ should not be created by read-only command");
    assert!(!repo.path().join("proj/sources").exists(), "sources/ should not be created by read-only command");
    assert!(!repo.path().join("proj/assets").exists(), "assets/ should not be created by read-only command");

    // Configured but empty dirs should also not be created by read-only
    assert!(!repo.path().join("proj/entries").exists(), "entries/ should not be created by read-only command");
    assert!(!repo.path().join("proj/refs").exists(), "refs/ should not be created by read-only command");
}

// ============================================================================
// US3: Compatibility — old projects continue working
// ============================================================================

/// T038: Omitted layout uses default directories.
#[test]
fn omitted_layout_uses_defaults() {
    let repo = common::setup_repo();
    common::write_mind_yaml(&repo, "proj", "schema: '1'\n");

    let output = mf()
        .current_dir(repo.path().join("proj"))
        .args(["config", "show", "--output-format", "json"])
        .output()
        .expect("command runs");
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let layout = &parsed["layout"];
    assert_eq!(layout["articles"], "docs");
    assert_eq!(layout["sources"], "sources");
    assert_eq!(layout["assets"], "assets");
    assert_eq!(layout["templates"], "templates");
    assert_eq!(layout["build_output"], "outputs");
}

/// T039: `paths.docs`, `paths.sources`, `paths.assets` map to effective layout.
#[test]
fn paths_compat_maps_to_effective_layout() {
    let repo = common::setup_repo();
    common::write_mind_yaml(&repo, "proj", "schema: '1'\npaths:\n  docs: notes\n  sources: refs\n  assets: media\n");

    let output = mf()
        .current_dir(repo.path().join("proj"))
        .args(["config", "show", "--output-format", "json"])
        .output()
        .expect("command runs");
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let layout = &parsed["layout"];
    assert_eq!(layout["articles"], "notes", "paths.docs should map to layout.articles");
    assert_eq!(layout["sources"], "refs", "paths.sources should map to layout.sources");
    assert_eq!(layout["assets"], "media", "paths.assets should map to layout.assets");
}

/// T040: `build.output_dir` maps to effective `layout.build_output`.
#[test]
fn build_output_dir_maps_to_layout_build_output() {
    let repo = common::setup_repo();
    common::write_mind_yaml(&repo, "proj", "schema: '1'\nbuild:\n  output_dir: dist\n");

    let output = mf()
        .current_dir(repo.path().join("proj"))
        .args(["config", "show", "--output-format", "json"])
        .output()
        .expect("command runs");
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let layout = &parsed["layout"];
    assert_eq!(layout["build_output"], "dist");
}

/// T041: No implicit migration — old files are not moved when layout changes.
#[test]
fn layout_change_does_not_move_existing_files() {
    let repo = common::setup_repo();
    // Start with default layout
    common::write_mind_yaml(&repo, "proj", "schema: '1'\n");

    // Create old-style article
    mf().current_dir(repo.path().join("proj")).args(["article", "new", "Old Doc"]).assert().success();

    let old_path = repo.path().join("proj/docs/old-doc/01-opening.md");
    assert!(old_path.exists(), "old doc should exist in docs/");

    // Now change layout to use articles/ instead of docs/
    common::write_mind_yaml(&repo, "proj", "schema: '1'\nlayout:\n  articles: articles\n");

    // Run a read-only command — this should NOT move files
    mf().current_dir(repo.path().join("proj")).args(["config", "show", "--output-format", "json"]).assert().success();

    // The old file should still be in docs/, not moved to articles/
    assert!(old_path.exists(), "old file in docs/ should still exist");
    assert!(
        !repo.path().join("proj/articles/old-doc/01-opening.md").exists(),
        "old file should NOT be moved to articles/ directory"
    );
}

/// T042: Terms file is accepted without `layout.terms`.
#[test]
fn terms_file_is_separate_from_layout() {
    let repo = common::setup_repo();
    common::write_mind_yaml(&repo, "proj", "schema: '1'\nlayout:\n  articles: entries\n");

    // Write a terms file at the project level
    fs::write(
        repo.path().join("proj/minds-terms.yaml"),
        "schema: '1'\nterms:\n  - term: API\n    definition: Application Programming Interface\n    corrections: []\n",
    )
    .unwrap();

    // Config show should not include terms in layout
    let output = mf()
        .current_dir(repo.path().join("proj"))
        .args(["config", "show", "--output-format", "json"])
        .output()
        .expect("command runs");
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let layout = &parsed["layout"];
    assert!(layout.get("terms").is_none(), "layout should not contain terms");
    assert_eq!(layout["articles"], "entries");
    assert!(layout.get("sources").is_some(), "layout should have sources");
    assert!(layout.get("assets").is_some(), "layout should have assets");
    assert!(layout.get("templates").is_some(), "layout should have templates");
    assert!(layout.get("build_output").is_some(), "layout should have build_output");
}

// ============================================================================
// US4: Validation diagnostics — clear errors for invalid layouts
// ============================================================================

/// T048: Duplicate layout paths across categories → error.
#[test]
fn duplicate_layout_paths_error() {
    let repo = common::setup_repo();
    common::write_mind_yaml(&repo, "proj", "schema: '1'\nlayout:\n  articles: content\n  sources: content\n");

    let output = mf().current_dir(repo.path().join("proj")).args(["config", "show"]).output().expect("command runs");
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("layout.articles"), "error should mention layout.articles:\n{stderr}");
    assert!(stderr.contains("layout.sources"), "error should mention layout.sources:\n{stderr}");
    assert!(
        stderr.contains("same path") || stderr.contains("content"),
        "error should mention the duplicate:\n{stderr}"
    );
}

/// T049: Empty or whitespace-only layout value → error.
#[test]
fn empty_layout_value_error() {
    let repo = common::setup_repo();
    common::write_mind_yaml(&repo, "proj", "schema: '1'\nlayout:\n  articles: '  '\n");

    let output = mf().current_dir(repo.path().join("proj")).args(["config", "show"]).output().expect("command runs");
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("layout.articles"), "error should mention the category:\n{stderr}");
    assert!(stderr.contains("empty"), "error should mention empty/whitespace:\n{stderr}");
}

/// T050: Absolute layout path → error.
#[test]
fn absolute_layout_path_error() {
    let repo = common::setup_repo();
    common::write_mind_yaml(&repo, "proj", "schema: '1'\nlayout:\n  articles: /etc/passwd\n");

    let output = mf().current_dir(repo.path().join("proj")).args(["config", "show"]).output().expect("command runs");
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("layout.articles"), "error should mention the category:\n{stderr}");
    assert!(stderr.contains("absolute"), "error should mention absolute path:\n{stderr}");
}

/// T051: Project-boundary escaping layout path → error.
#[test]
fn escaping_layout_path_error() {
    let repo = common::setup_repo();
    common::write_mind_yaml(&repo, "proj", "schema: '1'\nlayout:\n  sources: ../outside\n");

    let output = mf().current_dir(repo.path().join("proj")).args(["config", "show"]).output().expect("command runs");
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("layout.sources"), "error should mention the category:\n{stderr}");
    assert!(stderr.contains("escapes"), "error should mention escaping:\n{stderr}");
}

/// T052: Layout value pointing to an existing file → error.
#[test]
fn layout_value_is_existing_file_error() {
    let repo = common::setup_repo();
    common::write_mind_yaml(&repo, "proj", "schema: '1'\n");

    // Create a regular file that conflicts with the layout value
    fs::write(repo.path().join("proj/myfile"), b"hello").unwrap();

    common::write_mind_yaml(&repo, "proj", "schema: '1'\nlayout:\n  articles: myfile\n");

    let output = mf().current_dir(repo.path().join("proj")).args(["config", "show"]).output().expect("command runs");
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("layout.articles"), "error should mention the category:\n{stderr}");
    assert!(stderr.contains("file"), "error should mention that path is a file:\n{stderr}");
}

/// T053: Invalid layout produces usage error (exit code 2).
#[test]
fn invalid_layout_produces_usage_error() {
    let repo = common::setup_repo();
    common::write_mind_yaml(&repo, "proj", "schema: '1'\nlayout:\n  articles: /abs/path\n");

    let output = mf().current_dir(repo.path().join("proj")).args(["config", "show"]).output().expect("command runs");
    assert!(!output.status.success());

    // Usage errors should have exit code 2
    assert_eq!(output.status.code(), Some(2), "invalid layout should be exit code 2");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("layout.articles"), "should mention field:\n{stderr}");
}

// ============================================================================
// US3 additional: canon+paths conflict — canonical wins
// ============================================================================

/// When canonical `layout` and historical `paths` disagree, canonical value wins.
#[test]
fn canonical_layout_wins_over_paths_with_diagnostic() {
    let repo = common::setup_repo();
    common::write_mind_yaml(&repo, "proj", "schema: '1'\nlayout:\n  articles: canon\npaths:\n  docs: compat\n");

    let output = mf()
        .current_dir(repo.path().join("proj"))
        .args(["config", "show", "--output-format", "json"])
        .output()
        .expect("command runs");
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let layout = &parsed["layout"];
    assert_eq!(layout["articles"], "canon", "canonical layout should win over paths");
}

// ============================================================================
// US3 additional: `paths.archive` not in effective Layout
// ============================================================================

/// `paths.archive` is not a layout category and should not appear in layout output.
#[test]
fn paths_archive_not_in_effective_layout() {
    let repo = common::setup_repo();
    common::write_mind_yaml(&repo, "proj", "schema: '1'\npaths:\n  archive: _old\n");

    let output = mf()
        .current_dir(repo.path().join("proj"))
        .args(["config", "show", "--output-format", "json"])
        .output()
        .expect("command runs");
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let layout = &parsed["layout"];
    assert!(layout.get("archive").is_none(), "archive should not be a layout category");
}

// ============================================================================
// US4 additional: validation runs before command side effects
// ============================================================================

/// Invalid layout should prevent write commands from executing.
#[test]
fn invalid_layout_blocks_article_creation() {
    let repo = common::setup_repo();
    common::write_mind_yaml(&repo, "proj", "schema: '1'\nlayout:\n  articles: '   '\n");

    let output = mf()
        .current_dir(repo.path().join("proj"))
        .args(["article", "new", "Should Fail"])
        .output()
        .expect("command runs");
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("layout.articles"), "validation should block the command:\n{stderr}");
    assert!(stderr.contains("empty"), "should mention empty value:\n{stderr}");

    // No directory should have been created
    assert!(!repo.path().join("proj/docs").exists(), "no dir should be created on invalid layout");
}

/// Project show should not hide invalid layout diagnostics.
#[test]
fn invalid_layout_blocks_project_show() {
    let repo = common::setup_repo();
    common::write_mind_yaml(&repo, "proj", "schema: '1'\nlayout:\n  sources: ../outside\n");

    let output = mf().current_dir(repo.path()).args(["project", "show", "proj"]).output().expect("command runs");
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("layout.sources"), "project show should report invalid layout:\n{stderr}");
    assert!(stderr.contains("escapes"), "should mention project-boundary escape:\n{stderr}");
}
