use assert_cmd::Command;

mod common;

fn run(args: &[&str]) -> (String, String, i32) {
    let output = Command::cargo_bin("mf").expect("binary exists").args(args).output().expect("command runs");
    (
        String::from_utf8(output.stdout).expect("stdout utf8"),
        String::from_utf8(output.stderr).expect("stderr utf8"),
        output.status.code().unwrap_or_default(),
    )
}

// ═══════════════════════════════════════════════════════════════════════════
// article new (FR-020)
// ═══════════════════════════════════════════════════════════════════════════

// ---------------------------------------------------------------------------
// T102: article new TYPE TITLE (mind form, no warn)
// ---------------------------------------------------------------------------

#[test]
fn article_new_type_title_succeeds() {
    let dir = common::setup_repo();
    common::create_project(&dir, "alpha");

    let (stdout, stderr, code) = run(&[
        "--root",
        &dir.path().to_string_lossy(),
        "--format",
        "json",
        "article",
        "new",
        "blog",
        "My Article",
        "-p",
        "alpha",
    ]);

    assert_eq!(code, 0, "should succeed, stderr: {stderr:?}");
    assert!(stderr.is_empty(), "stderr should be clean: {stderr:?}");

    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["data"]["type"], "blog");
    assert!(parsed["data"]["filename"].as_str().unwrap_or("").contains("my-article"));
}

// ---------------------------------------------------------------------------
// T103: article new TITLE only (old form → usage error)
// ---------------------------------------------------------------------------

#[test]
fn article_new_title_only_errors() {
    let dir = common::setup_repo();
    common::create_project(&dir, "alpha");

    // clap requires TYPE and TITLE, so single positional should error
    let (_stdout, stderr, code) =
        run(&["--root", &dir.path().to_string_lossy(), "article", "new", "MyArticle", "-p", "alpha"]);

    // clap should report a usage error (exit 2)
    assert_eq!(code, 2, "should error with exit code 2: stderr={stderr:?}");
}

// ═══════════════════════════════════════════════════════════════════════════
// publish update (FR-021)
// ═══════════════════════════════════════════════════════════════════════════

// ---------------------------------------------------------------------------
// T104: publish update --set k=v (mind form, no warn)
// ---------------------------------------------------------------------------

#[test]
fn publish_update_set_no_warning() {
    let dir = common::setup_repo();
    common::create_project(&dir, "alpha");

    // --set is the mind primary form — no deprecation warning
    // (command will fail after dispatch because article/target don't exist,
    //  but we can verify stderr has no [deprecated])
    let (_stdout, stderr, _code) = run(&[
        "--root",
        &dir.path().to_string_lossy(),
        "publish",
        "update",
        "my-article",
        "--target",
        "local",
        "--set",
        "status=draft",
        "-p",
        "alpha",
    ]);
    assert!(!stderr.contains("[deprecated]"), "stderr should have NO deprecation: {stderr:?}");
}

// ---------------------------------------------------------------------------
// T105: publish update --status --target-url (D1a+D1b warnings)
// ---------------------------------------------------------------------------

#[test]
fn publish_update_status_target_url_deprecated() {
    let dir = common::setup_repo();
    common::create_project(&dir, "alpha");

    let (_stdout, stderr, _code) = run(&[
        "--root",
        &dir.path().to_string_lossy(),
        "publish",
        "update",
        "my-article",
        "--target",
        "local",
        "--status",
        "draft",
        "--target-url",
        "https://example.com",
        "-p",
        "alpha",
    ]);
    assert!(stderr.contains("[deprecated]"), "stderr should have deprecation: {stderr:?}");
    assert!(stderr.contains("--status"), "should mention --status: {stderr:?}");
    assert!(stderr.contains("--target-url"), "should mention --target-url: {stderr:?}");
}

// ═══════════════════════════════════════════════════════════════════════════
// source add (FR-022)
// ═══════════════════════════════════════════════════════════════════════════

// ---------------------------------------------------------------------------
// T106: source add --source-kind yuque (mind form, no warn)
// ---------------------------------------------------------------------------

#[test]
fn source_add_source_kind_no_warning() {
    let dir = common::setup_repo();
    common::create_project(&dir, "alpha");

    let (stdout, stderr, code) = run(&[
        "--root",
        &dir.path().to_string_lossy(),
        "--format",
        "json",
        "source",
        "add",
        "--name",
        "test-source",
        "--source-kind",
        "yuque",
        "https://example.com/doc",
        "-p",
        "alpha",
    ]);
    assert_eq!(code, 0, "should succeed, stderr: {stderr:?}");
    assert!(!stderr.contains("[deprecated]"), "stderr should have NO deprecation: {stderr:?}");

    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["status"], "ok");
}

// ---------------------------------------------------------------------------
// T107: source add --type yuque (D2a warning — source-kind value)
// ---------------------------------------------------------------------------

#[test]
fn source_add_type_yuque_deprecated() {
    let dir = common::setup_repo();
    common::create_project(&dir, "alpha");

    let (_stdout, stderr, _code) = run(&[
        "--root",
        &dir.path().to_string_lossy(),
        "source",
        "add",
        "--name",
        "test-source",
        "--type",
        "auto",
        "https://example.com/doc",
        "-p",
        "alpha",
    ]);
    // Deprecation is emitted in dispatch before handle_add
    assert!(stderr.contains("[deprecated]"), "stderr should have deprecation: {stderr:?}");
    assert!(stderr.contains("--type"), "stderr should mention --type: {stderr:?}");
}

// ---------------------------------------------------------------------------
// T108: source add --type pdf (D2b warning — file-kind value)
// ---------------------------------------------------------------------------

#[test]
fn source_add_type_pdf_deprecated() {
    let dir = common::setup_repo();
    common::create_project(&dir, "alpha");

    // pdf is a valid CliSourceKind value; deprecation fires for --type itself
    let (_stdout, stderr, _code) = run(&[
        "--root",
        &dir.path().to_string_lossy(),
        "source",
        "add",
        "--name",
        "test-source",
        "--type",
        "pdf",
        "https://example.com/doc",
        "-p",
        "alpha",
    ]);
    // Note: pdf + URL will fail the service call (pdf requires local file),
    // but the deprecation warning is emitted before that
    assert!(stderr.contains("[deprecated]"), "stderr should have deprecation: {stderr:?}");
    assert!(stderr.contains("--type"), "stderr should mention --type: {stderr:?}");
}

// ---------------------------------------------------------------------------
// T109: source add --type unknown (usage error)
// ---------------------------------------------------------------------------

#[test]
fn source_add_type_unknown_errors() {
    let dir = common::setup_repo();
    common::create_project(&dir, "alpha");

    // clap should reject invalid value for value enum
    let (_stdout, stderr, code) = run(&[
        "--root",
        &dir.path().to_string_lossy(),
        "source",
        "add",
        "--name",
        "test-source",
        "--type",
        "bogus",
        "https://example.com/doc",
        "-p",
        "alpha",
    ]);
    assert_eq!(code, 2, "should error with exit 2: stderr={stderr:?}");
}

// ═══════════════════════════════════════════════════════════════════════════
// source remove (FR-023)
// ═══════════════════════════════════════════════════════════════════════════

// ---------------------------------------------------------------------------
// T110: source remove PATH (mind form, no warn)
// ---------------------------------------------------------------------------

#[test]
fn source_remove_path_no_warning() {
    let dir = common::setup_repo();
    common::create_project(&dir, "alpha");

    // PATH form (contains /) — no deprecation warning
    let (_stdout, stderr, _code) =
        run(&["--root", &dir.path().to_string_lossy(), "source", "remove", "sources/yuque/my-doc.md", "-p", "alpha"]);
    // Even though the source doesn't exist and command will fail,
    // the deprecation decision happens before service call
    assert!(!stderr.contains("[deprecated]"), "stderr should have NO deprecation: {stderr:?}");
}

// ---------------------------------------------------------------------------
// T111: source remove NAME (D3 warning)
// ---------------------------------------------------------------------------

#[test]
fn source_remove_name_deprecated() {
    let dir = common::setup_repo();
    common::create_project(&dir, "alpha");

    // NAME form (no /, doesn't start with sources) — D3 deprecation
    let (_stdout, stderr, _code) =
        run(&["--root", &dir.path().to_string_lossy(), "source", "remove", "my-doc", "-p", "alpha"]);
    assert!(stderr.contains("[deprecated]"), "stderr should have deprecation: {stderr:?}");
    assert!(stderr.contains("NAME"), "stderr should mention NAME: {stderr:?}");
}

// ═══════════════════════════════════════════════════════════════════════════
// asset add (FR-024)
// ═══════════════════════════════════════════════════════════════════════════

// ---------------------------------------------------------------------------
// T112: asset add --name --copy --tag (merge, no warn)
// ---------------------------------------------------------------------------

#[test]
fn asset_add_name_copy_tag_no_warning() {
    let dir = common::setup_repo();
    common::create_project(&dir, "alpha");
    // Create a file to add
    let test_file = dir.path().join("test-image.png");
    std::fs::write(&test_file, b"fake png content").unwrap();

    let (stdout, stderr, code) = run(&[
        "--root",
        &dir.path().to_string_lossy(),
        "--format",
        "json",
        "asset",
        "add",
        &test_file.to_string_lossy(),
        "--name",
        "my-image",
        "--tag",
        "screenshot",
        "-p",
        "alpha",
    ]);
    assert_eq!(code, 0, "should succeed, stderr: {stderr:?}");
    assert!(!stderr.contains("[deprecated]"), "stderr should have NO deprecation: {stderr:?}");

    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["data"]["name"], "my-image");
}

// ═══════════════════════════════════════════════════════════════════════════
// asset update (FR-025)
// ═══════════════════════════════════════════════════════════════════════════

// ---------------------------------------------------------------------------
// T113: asset update --set-url --channel (mind form)
// ---------------------------------------------------------------------------

#[test]
fn asset_update_set_url_channel_succeeds() {
    let dir = common::setup_repo();
    common::create_project(&dir, "alpha");

    let (stdout, stderr, code) = run(&[
        "--root",
        &dir.path().to_string_lossy(),
        "--format",
        "json",
        "asset",
        "update",
        "--set-url",
        "https://example.com/asset",
        "--channel",
        "yuque",
        "-p",
        "alpha",
    ]);
    assert_eq!(code, 0, "should succeed, stderr: {stderr:?}");
    assert!(!stderr.contains("[deprecated]"), "stderr should have NO deprecation: {stderr:?}");

    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["data"]["url"], "https://example.com/asset");
}

// ---------------------------------------------------------------------------
// T114: asset update --all (mf form)
// ---------------------------------------------------------------------------

#[test]
fn asset_update_all_succeeds() {
    let dir = common::setup_repo();
    common::create_project(&dir, "alpha");

    let (stdout, stderr, code) =
        run(&["--root", &dir.path().to_string_lossy(), "--format", "json", "asset", "update", "--all", "-p", "alpha"]);
    assert_eq!(code, 0, "should succeed, stderr: {stderr:?}");
    assert!(!stderr.contains("[deprecated]"), "stderr should have NO deprecation: {stderr:?}");

    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["status"], "ok");
}

// ═══════════════════════════════════════════════════════════════════════════
// term learn (FR-026)
// ═══════════════════════════════════════════════════════════════════════════

// ---------------------------------------------------------------------------
// T115: term learn --term --alias (mind form, no warn)
// ---------------------------------------------------------------------------

#[test]
fn term_learn_term_alias_no_warning() {
    let dir = common::setup_repo();
    common::create_project(&dir, "alpha");
    let index_yaml = r#"schema_version: '1'
terms:
  - term: Mind Repo
    aliases: []
    corrections: []
"#;
    common::write_index(&dir, "alpha", index_yaml);

    let (_stdout, stderr, code) = run(&[
        "--root",
        &dir.path().to_string_lossy(),
        "term",
        "learn",
        "--term",
        "Mind Repo",
        "--alias",
        "old-mr",
        "-p",
        "alpha",
    ]);
    assert_eq!(code, 0, "should succeed, stderr: {stderr:?}");
    assert!(!stderr.contains("[deprecated]"), "stderr should have NO deprecation: {stderr:?}");
}

// ---------------------------------------------------------------------------
// T116: term learn --original --correct (D4a+D4b warnings)
// ---------------------------------------------------------------------------

#[test]
fn term_learn_original_correct_deprecated() {
    let dir = common::setup_repo();
    common::create_project(&dir, "alpha");
    let index_yaml = r#"schema_version: '1'
terms:
  - term: Mind Repo
    aliases: []
    corrections: []
"#;
    common::write_index(&dir, "alpha", index_yaml);

    let (_stdout, stderr, _code) = run(&[
        "--root",
        &dir.path().to_string_lossy(),
        "term",
        "learn",
        "--original",
        "old-mr",
        "--correct",
        "Mind Repo",
        "-p",
        "alpha",
    ]);
    assert!(stderr.contains("[deprecated]"), "stderr should have deprecation: {stderr:?}");
    assert!(stderr.contains("--original"), "stderr should mention --original: {stderr:?}");
    assert!(stderr.contains("--correct"), "stderr should mention --correct: {stderr:?}");
}

// ═══════════════════════════════════════════════════════════════════════════
// term lint (FR-027)
// ═══════════════════════════════════════════════════════════════════════════

// ---------------------------------------------------------------------------
// T117: term lint path/to/file.md (with PATH, no warn)
// ---------------------------------------------------------------------------

#[test]
fn term_lint_with_path_succeeds() {
    let dir = common::setup_repo();
    common::create_project(&dir, "alpha");
    // Add a term with corrections
    let index_yaml = r#"schema_version: '1'
terms:
  - term: Mind Repo
    definition: The mind repository
    aliases:
      - mr
    corrections:
      - original: mindrepo
        correct: Mind Repo
      - original: mind-repo
        correct: Mind Repo
"#;
    common::write_index(&dir, "alpha", index_yaml);
    // Create a doc file with the incorrect form
    let doc_path = dir.path().join("alpha").join("docs");
    std::fs::create_dir_all(&doc_path).unwrap();
    std::fs::write(doc_path.join("test.md"), "uses mindrepo and mind-repo").unwrap();

    // Path is relative to project (alpha) root
    let (stdout, stderr, code) =
        run(&["--root", &dir.path().to_string_lossy(), "term", "lint", "docs/test.md", "-p", "alpha"]);
    assert_eq!(code, 1, "should exit 1 (findings), stderr: {stderr:?}");
    assert!(!stderr.contains("[deprecated]"), "stderr should have NO deprecation: {stderr:?}");
    assert!(stdout.contains("mindrepo"), "stdout should mention finding: {stdout}");
}

// ---------------------------------------------------------------------------
// T118: term lint (without PATH, no warn)
// ---------------------------------------------------------------------------

#[test]
fn term_lint_without_path_succeeds() {
    let dir = common::setup_repo();
    common::create_project(&dir, "alpha");
    // Add a term with corrections
    let index_yaml = r#"schema_version: '1'
terms:
  - term: Mind Repo
    definition: The mind repository
    aliases: []
    corrections:
      - original: mindrepo
        correct: Mind Repo
"#;
    common::write_index(&dir, "alpha", index_yaml);
    // Create a doc file with the incorrect form
    let doc_path = dir.path().join("alpha").join("docs");
    std::fs::create_dir_all(&doc_path).unwrap();
    std::fs::write(doc_path.join("test.md"), "uses mindrepo here").unwrap();

    let (stdout, stderr, code) = run(&["--root", &dir.path().to_string_lossy(), "term", "lint", "-p", "alpha"]);
    assert_eq!(code, 1, "should exit 1 (findings), stderr: {stderr:?}");
    assert!(!stderr.contains("[deprecated]"), "stderr should have NO deprecation: {stderr:?}");
    assert!(stdout.contains("mindrepo"), "stdout should mention finding: {stdout}");
}
