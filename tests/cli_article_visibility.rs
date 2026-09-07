use assert_cmd::Command;

mod common;

// ── mf article lint: mind-forge-visibility validation (spec 073, FR-006/FR-007) ──

#[test]
fn article_lint_reports_invalid_visibility_value() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let docs = repo.path().join("my-project/docs");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(docs.join("bad-visibility.md"), "---\nmind-forge-visibility: internal\n---\n\n# Title\n\nBody.\n")
        .unwrap();

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "lint"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(1), "an error-severity issue must fail the lint exit code");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("mind_forge_visibility_invalid"), "stdout should name the issue kind:\n{stdout}");
    assert!(stdout.contains("internal"), "stdout should name the invalid value:\n{stdout}");
}

#[test]
fn article_lint_reports_invalid_visibility_value_json() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let docs = repo.path().join("my-project/docs");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(docs.join("bad-visibility.md"), "---\nmind-forge-visibility: privat\n---\n\n# Title\n\nBody.\n")
        .unwrap();

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["--output", "json", "article", "lint"])
        .output()
        .expect("command runs");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let issues = parsed["data"]["issues"].as_array().expect("issues array");
    let issue = issues
        .iter()
        .find(|i| i["kind"] == "mind_forge_visibility_invalid")
        .expect("mind_forge_visibility_invalid issue present");
    assert_eq!(issue["severity"], "error");
    assert_eq!(issue["fixable"], false);
    assert_eq!(issue["path"], "docs/bad-visibility.md");
}

#[test]
fn article_lint_reports_private_title_block() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let docs = repo.path().join("my-project/docs");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(docs.join("private-title.md"), "---\nmind-forge-visibility: private\n---\n\n# Title\n\nBody.\n")
        .unwrap();

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["--output", "json", "article", "lint"])
        .output()
        .expect("command runs");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let issues = parsed["data"]["issues"].as_array().expect("issues array");
    let issue = issues
        .iter()
        .find(|i| i["kind"] == "mind_forge_private_title_block")
        .expect("mind_forge_private_title_block issue present");
    assert_eq!(issue["severity"], "error");
    assert_eq!(issue["fixable"], false);
}

#[test]
fn article_lint_fix_does_not_alter_visibility_issues() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let docs = repo.path().join("my-project/docs");
    std::fs::create_dir_all(&docs).unwrap();
    let content = "---\nmind-forge-visibility: private\n---\n\n# Title\n\nBody.\n";
    std::fs::write(docs.join("private-title.md"), content).unwrap();

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "lint", "--fix"])
        .output()
        .expect("command runs");

    // --fix must not have rewritten the file to "resolve" the issue.
    let after = std::fs::read_to_string(docs.join("private-title.md")).unwrap();
    assert_eq!(after, content, "mind-forge-visibility issues are not auto-fixable");
}

#[test]
fn article_lint_no_issues_for_valid_visibility() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let docs = repo.path().join("my-project/docs");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(docs.join("fine.md"), "---\nmind-forge-visibility: public\n---\n\n# Title\n\nBody.\n").unwrap();
    std::fs::write(docs.join("fine-no-key.md"), "# Title\n\nBody.\n").unwrap();

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["article", "lint"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.contains("mind_forge_visibility_invalid"));
    assert!(!stdout.contains("mind_forge_private_title_block"));
}

// ── spec 079 T017: lock in spec-077 fixes (#44/#48) with regression tests ──

/// #44: stripping a private callout must not leave more than one blank line
/// between the surrounding paragraphs (blank-line normalisation).
#[test]
fn build_strip_normalizes_to_single_blank_line() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_article_index(&repo, "my-project", "spacing");
    common::write_doc(&repo, "my-project", "spacing", "# Spacing\n\nBefore.\n\n> [!mf-private]\n> Secret.\n\nAfter.\n");

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["build", "spacing"])
        .assert()
        .success();

    let content = std::fs::read_to_string(repo.path().join("my-project/outputs/spacing.md")).unwrap();
    assert!(!content.contains("\n\n\n"), "no more than one blank line must remain: {content:?}");
    assert!(
        content.contains("Before.\n\nAfter.\n"),
        "paragraphs must be separated by exactly one blank line: {content:?}"
    );
}

/// #48: a private callout with no leading blank line (immediately following
/// another line of prose) must still be recognised and stripped.
#[test]
fn build_strips_private_callout_with_no_leading_blank_line() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    common::write_article_index(&repo, "my-project", "no-lead-blank");
    common::write_doc(
        &repo,
        "my-project",
        "no-lead-blank",
        "# No Lead Blank\n\nBefore.\n> [!mf-private]\n> Secret.\n\nAfter.\n",
    );

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(["build", "no-lead-blank"])
        .assert()
        .success();

    let content = std::fs::read_to_string(repo.path().join("my-project/outputs/no-lead-blank.md")).unwrap();
    assert!(!content.contains("Secret"), "callout must be stripped even with no leading blank line: {content:?}");
    assert!(content.contains("Before."), "prose before the callout must survive: {content:?}");
    assert!(content.contains("After."), "prose after the callout must survive: {content:?}");
}
