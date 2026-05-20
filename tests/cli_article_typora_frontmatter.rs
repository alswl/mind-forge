mod common;

use assert_cmd::Command;

/// Helper: run `mf article new` with --json and return parsed stdout.
fn article_new_json(repo: &common::TempDir, project: &str, args: &[&str]) -> serde_json::Value {
    let mut cmd_args = vec!["article", "new"];
    cmd_args.extend_from_slice(args);
    cmd_args.push("--json");
    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join(project))
        .args(&cmd_args)
        .output()
        .expect("command runs");
    assert!(output.status.success(), "article new failed: {}", String::from_utf8_lossy(&output.stderr));
    serde_json::from_slice(&output.stdout).expect("valid JSON")
}

// ── T014: Default directory article Typora front matter ──

#[test]
fn directory_article_injects_typora_front_matter() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let json = article_new_json(&repo, "my-project", &["My Post"]);
    let data = &json["data"];

    // JSON contract
    assert_eq!(data["typora_front_matter_injected"], true);
    assert!(data["typora_copy_images_to"].is_string());
    assert!(!data["typora_copy_images_to"].as_str().unwrap().is_empty());

    // File content
    let md_path = repo.path().join("my-project/docs/my-post/01-opening.md");
    let content = std::fs::read_to_string(&md_path).unwrap();
    assert!(content.contains("typora-copy-images-to:"), "file should contain typora-copy-images-to");
    assert!(content.contains("typora-copy-images-to: ../../assets\n"));
    assert!(content.starts_with("---\n"), "file should start with YAML front matter block");
}

#[test]
fn directory_article_injects_typora_front_matter_into_every_block() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let json = article_new_json(&repo, "my-project", &["Architecture Note", "--template", "arch"]);
    let files = json["data"]["files"].as_array().expect("files should be an array");
    assert!(files.len() > 1, "arch template should create multiple block files");
    assert_eq!(json["data"]["typora_copy_images_to"], "../../assets");

    for file in files {
        let file = file.as_str().expect("file entry should be a string");
        let md_path = repo.path().join("my-project/docs/architecture-note").join(file);
        let content = std::fs::read_to_string(&md_path).unwrap();
        assert!(content.starts_with("---\n"), "{file} should start with YAML front matter");
        assert!(
            content.contains("typora-copy-images-to: ../../assets\n"),
            "{file} should contain the directory article assets path"
        );
    }
}

// ── T015: Single-file article Typora front matter ──

#[test]
fn file_article_injects_typora_front_matter() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let json = article_new_json(&repo, "my-project", &["Flash Note", "--file"]);
    let data = &json["data"];

    assert_eq!(data["typora_front_matter_injected"], true);
    assert!(data["typora_copy_images_to"].is_string());

    let md_path = repo.path().join("my-project/docs/flash-note.md");
    let content = std::fs::read_to_string(&md_path).unwrap();
    assert!(content.contains("typora-copy-images-to:"), "file should contain typora-copy-images-to");
    assert!(content.contains("typora-copy-images-to: ../assets\n"));
    assert!(content.starts_with("---\n"));
}

// ── T016: JSON envelope fields ──

#[test]
fn json_envelope_includes_typora_fields() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let json = article_new_json(&repo, "my-project", &["JSON Test"]);
    let data = &json["data"];

    assert_eq!(data["typora_front_matter_injected"], true);
    let path_val = data["typora_copy_images_to"].as_str().unwrap();
    assert!(path_val.contains("assets"), "typora_copy_images_to should reference assets dir");
}

// ── T017: Custom layout.assets relative path ──

#[test]
fn custom_assets_path_used_in_front_matter() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    // Override assets to "media"
    let mind_yaml = "schema_version: '1'\nlayout:\n  assets: media\n";
    std::fs::write(repo.path().join("my-project/mind.yaml"), mind_yaml).unwrap();

    let json = article_new_json(&repo, "my-project", &["Daily Note"]);
    let data = &json["data"];

    assert_eq!(data["typora_front_matter_injected"], true);
    assert_eq!(data["typora_copy_images_to"], "../../media");

    let md_path = repo.path().join("my-project/docs/daily-note/01-opening.md");
    let content = std::fs::read_to_string(&md_path).unwrap();
    assert!(content.contains("typora-copy-images-to:"), "file should contain typora-copy-images-to");
    assert!(content.contains("typora-copy-images-to: ../../media\n"), "file should reference media dir");
}

#[test]
fn custom_template_existing_front_matter_is_merged() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    let template_dir = repo.path().join("my-project/templates");
    std::fs::create_dir_all(&template_dir).unwrap();
    std::fs::write(template_dir.join("with-front-matter.md"), "---\ntitle: From Template\n---\n# {title}\n\nBody\n")
        .unwrap();

    article_new_json(
        &repo,
        "my-project",
        &["Merged Template", "--template", "templates/with-front-matter.md", "--file"],
    );

    let md_path = repo.path().join("my-project/docs/merged-template.md");
    let content = std::fs::read_to_string(&md_path).unwrap();
    assert_eq!(content.matches("\n---").count(), 1, "should not create a second front matter block");
    assert!(content.starts_with("---\n"));
    assert!(content.contains("title: From Template\n"));
    assert!(content.contains("typora-copy-images-to: ../assets\n"));
}

#[test]
fn custom_template_existing_typora_value_is_preserved() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    let template_dir = repo.path().join("my-project/templates");
    std::fs::create_dir_all(&template_dir).unwrap();
    std::fs::write(
        template_dir.join("with-typora.md"),
        "---\ntypora-copy-images-to: custom-images\ntitle: From Template\n---\n# {title}\n",
    )
    .unwrap();

    article_new_json(&repo, "my-project", &["Preserved Template", "--template", "templates/with-typora.md", "--file"]);

    let md_path = repo.path().join("my-project/docs/preserved-template.md");
    let content = std::fs::read_to_string(&md_path).unwrap();
    assert_eq!(content.matches("typora-copy-images-to:").count(), 1);
    assert!(content.contains("typora-copy-images-to: custom-images\n"));
    assert!(!content.contains("typora-copy-images-to: ../assets\n"));
}

// ── US2 tests ──

/// Helper: write mind.yaml and run article new --json.
fn article_new_with_config(repo: &common::TempDir, project: &str, mind_yaml: &str, args: &[&str]) -> serde_json::Value {
    std::fs::write(repo.path().join(project).join("mind.yaml"), mind_yaml).unwrap();
    article_new_json(repo, project, args)
}

// ── T024: enabled: false omits Typora front matter ──

#[test]
fn disabled_plugin_omits_typora_front_matter() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let config = "schema_version: '1'\nplugins:\n  typora-front-matter:\n    enabled: false\n";
    let json = article_new_with_config(&repo, "my-project", config, &["No Typora"]);
    let data = &json["data"];

    assert_eq!(data["typora_front_matter_injected"], false);
    assert!(data["typora_copy_images_to"].is_null());

    let md_path = repo.path().join("my-project/docs/no-typora/01-opening.md");
    let content = std::fs::read_to_string(&md_path).unwrap();
    assert!(!content.contains("typora-copy-images-to:"), "file should NOT contain typora-copy-images-to when disabled");
}

// ── T025: explicit enabled: true matches default ──

#[test]
fn explicit_enabled_true_matches_default() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let config = "schema_version: '1'\nplugins:\n  typora-front-matter:\n    enabled: true\n";
    let json = article_new_with_config(&repo, "my-project", config, &["Explicit Enable"]);
    let data = &json["data"];

    assert_eq!(data["typora_front_matter_injected"], true);
    assert!(data["typora_copy_images_to"].is_string());

    let md_path = repo.path().join("my-project/docs/explicit-enable/01-opening.md");
    let content = std::fs::read_to_string(&md_path).unwrap();
    assert!(
        content.contains("typora-copy-images-to:"),
        "file should contain typora-copy-images-to with explicit enable"
    );
}

// ── T026: unknown plugins preserve default Typora behavior ──

#[test]
fn unknown_plugin_preserves_typora_default() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let config = "schema_version: '1'\nplugins:\n  some-other-plugin:\n    key: value\n";
    let json = article_new_with_config(&repo, "my-project", config, &["Unknown Plugin"]);
    let data = &json["data"];

    // Default behavior: Typora is enabled when its config is absent
    assert_eq!(data["typora_front_matter_injected"], true);
    assert!(data["typora_copy_images_to"].is_string());

    let md_path = repo.path().join("my-project/docs/unknown-plugin/01-opening.md");
    let content = std::fs::read_to_string(&md_path).unwrap();
    assert!(
        content.contains("typora-copy-images-to:"),
        "file should contain typora-copy-images-to when only unknown plugins are present"
    );
}

// ── T027: invalid enabled type produces readable error ──

#[test]
fn invalid_enabled_type_rejected() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    std::fs::write(
        repo.path().join("my-project/mind.yaml"),
        "schema_version: '1'\nplugins:\n  typora-front-matter:\n    enabled: \"yes\"\n",
    )
    .unwrap();

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "new", "Bad Config", "--json"])
        .output()
        .expect("command runs");

    assert!(!output.status.success(), "should fail with invalid enabled type");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("plugin")
            || stderr.contains("enabled")
            || stderr.contains("typora")
            || stderr.contains("invalid")
            || stderr.contains("parse error")
            || stderr.contains("boolean"),
        "error should mention the invalid field, got: {}",
        stderr
    );
}

// ══════════════════════════════════════════════════════════════════════
// US3: Existing articles are not modified
// ══════════════════════════════════════════════════════════════════════

/// Helper: read a Markdown file and return its content.
fn read_md(repo: &common::TempDir, project: &str, rel_path: &str) -> String {
    std::fs::read_to_string(repo.path().join(project).join(rel_path)).unwrap()
}

/// Helper: write a Markdown file in a project's docs/ directory.
fn write_existing_md(repo: &common::TempDir, project: &str, filename: &str, content: &str) {
    let docs_dir = repo.path().join(project).join("docs");
    std::fs::create_dir_all(&docs_dir).unwrap();
    std::fs::write(docs_dir.join(filename), content).unwrap();
}

// ── T035: list and lint do not rewrite existing articles ──

#[test]
fn list_does_not_rewrite_existing_article() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    write_existing_md(&repo, "my-project", "existing.md", "# Existing\n\nNo front matter here.\n");
    let original = read_md(&repo, "my-project", "docs/existing.md");

    // Run article list (which rebuilds the index)
    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "list"])
        .output()
        .expect("command runs");
    assert!(output.status.success(), "article list should succeed");

    let after = read_md(&repo, "my-project", "docs/existing.md");
    assert_eq!(after, original, "article list must not modify existing files");
}

#[test]
fn lint_does_not_rewrite_existing_article() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    write_existing_md(&repo, "my-project", "existing.md", "# Existing\n\nNo front matter.\n");
    let original = read_md(&repo, "my-project", "docs/existing.md");

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "lint"])
        .output()
        .expect("command runs");
    assert!(output.status.success(), "article lint should succeed");

    let after = read_md(&repo, "my-project", "docs/existing.md");
    assert_eq!(after, original, "article lint must not modify existing files");
}

// ── T036: new article creation does not modify existing articles ──

#[test]
fn new_article_does_not_touch_existing_with_custom_front_matter() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let existing_content = "---\nmf_term_lint: skip\n---\n# Existing\n\nBody.\n";
    write_existing_md(&repo, "my-project", "existing.md", existing_content);
    let original = read_md(&repo, "my-project", "docs/existing.md");

    // Create a new article (which triggers Typora injection on new files only)
    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "new", "Fresh Article", "--json"])
        .output()
        .expect("command runs");
    assert!(output.status.success(), "article new should succeed");

    let after = read_md(&repo, "my-project", "docs/existing.md");
    assert_eq!(after, original, "new article must not modify existing files");
}

// ── T037: build tolerates Typora front matter without rewriting source ──

#[test]
fn build_tolerates_typora_front_matter_without_rewriting() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    // Write an article with Typora front matter and an existing index entry
    let article_content = "---\ntypora-copy-images-to: ../assets\n---\n# Article\n\nBody.\n";
    write_existing_md(&repo, "my-project", "has-typora.md", article_content);

    // Pre-create mind-index.yaml with an entry for this article so build finds it
    common::write_article_index(&repo, "my-project", "has-typora");
    let original = read_md(&repo, "my-project", "docs/has-typora.md");

    // Run build
    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["build", "has-typora"])
        .output()
        .expect("command runs");
    assert!(output.status.success(), "build should succeed: {}", String::from_utf8_lossy(&output.stderr));

    let after = read_md(&repo, "my-project", "docs/has-typora.md");
    assert_eq!(after, original, "build must not modify the source file");
}
