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

/// Spec 065 FR-012: converting an article's shape (single-file <-> directory)
/// keeps a bound prompt's frontmatter and both projections consistent — no
/// spurious `orphan_prompt`/`missing_thinking` finding immediately after
/// conversion, without an explicit `mf article index`.
#[test]
fn article_convert_to_directory_keeps_prompt_and_thinking_consistent() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");

    std::fs::create_dir_all(project.join("docs")).unwrap();
    std::fs::create_dir_all(project.join("sources")).unwrap();
    std::fs::create_dir_all(project.join("assets")).unwrap();
    std::fs::create_dir_all(project.join("prompts")).unwrap();
    std::fs::create_dir_all(project.join("thinking")).unwrap();
    std::fs::write(project.join("docs/my-article.md"), "# My Article\n\nContent.\n").unwrap();
    std::fs::write(
        project.join("prompts/my-article.md"),
        "---\narticle: docs/my-article.md\nmode: research\n---\n\nBrief.\n",
    )
    .unwrap();
    std::fs::write(project.join("thinking/my-article.md"), "## Notes\n\nWorking ledger.\n").unwrap();

    let index_yaml = r#"schema: '1'
articles:
  - title: My Article
    project: alpha
    type: blog
    article_path: docs/my-article.md
    status: draft
    created_at: '2026-07-01T00:00:00Z'
    updated_at: '2026-07-01T00:00:00Z'
"#;
    common::write_index(&repo, "alpha", index_yaml);

    let (_o, e, c) = run(&["--root", &repo.path().to_string_lossy(), "article", "index", "--project", "alpha"]);
    assert_eq!(c, 0, "stderr: {e}");

    let (_stdout, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "article", "convert", "--to-directory", "--project", "alpha"]);
    assert_eq!(code, 0, "convert should succeed: stderr={stderr}");
    assert!(project.join("docs/my-article/01-opening.md").exists());

    // Prompt file unaffected (same key, docs/my-article -> docs/my-article
    // for article_output_stem), but its frontmatter binding is unchanged
    // since the stem is invariant across this specific conversion.
    let prompt_content = std::fs::read_to_string(project.join("prompts/my-article.md")).unwrap();
    assert!(prompt_content.contains("article:"), "{prompt_content}");

    // Projections immediately reflect the converted article — no re-index required.
    // Converting to a directory shape drops the `.md` suffix from the article path.
    let (show_stdout, show_stderr, show_code) = run(&[
        "--root",
        &repo.path().to_string_lossy(),
        "--output",
        "json",
        "article",
        "show",
        "docs/my-article",
        "--project",
        "alpha",
    ]);
    assert_eq!(show_code, 0, "stderr: {show_stderr}");
    let parsed: serde_json::Value = serde_json::from_str(&show_stdout).unwrap();
    assert_eq!(parsed["data"]["prompt"]["path"], "prompts/my-article.md");
    assert_eq!(parsed["data"]["prompt"]["binding_status"], "bound", "{show_stdout}");

    // No spurious lint findings after conversion.
    let (lint_stdout, lint_stderr, lint_code) =
        run(&["--root", &repo.path().to_string_lossy(), "--output", "json", "project", "lint", "--project", "alpha"]);
    let parsed: serde_json::Value = serde_json::from_str(&lint_stdout).unwrap();
    let issues = parsed["data"]["issues"].as_array().unwrap();
    let our_kinds = ["orphan_prompt", "duplicate_binding", "missing_thinking"];
    assert!(
        issues.iter().all(|i| !our_kinds.contains(&i["kind"].as_str().unwrap_or(""))),
        "no spurious prompt/thinking findings after conversion: {lint_stdout}"
    );
    assert_eq!(lint_code, 0, "stderr: {lint_stderr}");
}
