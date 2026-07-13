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

/// Spec 065 FR-012: renaming an article keeps the prompt file, thinking
/// file, and both projections consistent — no spurious `orphan_prompt` or
/// `missing_thinking` lint finding should appear immediately after rename,
/// without requiring an explicit `mf article index`.
#[test]
fn article_rename_keeps_prompt_and_thinking_consistent() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");

    std::fs::create_dir_all(project.join("docs")).unwrap();
    std::fs::create_dir_all(project.join("sources")).unwrap();
    std::fs::create_dir_all(project.join("assets")).unwrap();
    std::fs::create_dir_all(project.join("prompts")).unwrap();
    std::fs::create_dir_all(project.join("thinking")).unwrap();
    std::fs::write(project.join("docs/old-name.md"), "# Old Name\n").unwrap();
    std::fs::write(
        project.join("prompts/old-name.md"),
        "---\narticle: docs/old-name.md\nmode: research\n---\n\nBrief.\n",
    )
    .unwrap();
    std::fs::write(project.join("thinking/old-name.md"), "## Notes\n\nWorking ledger.\n").unwrap();

    let index_yaml = r#"schema: '1'
articles:
  - title: Old Name
    project: alpha
    type: blog
    article_path: docs/old-name.md
    status: draft
    created_at: '2026-07-01T00:00:00Z'
    updated_at: '2026-07-01T00:00:00Z'
"#;
    common::write_index(&repo, "alpha", index_yaml);

    // Reconcile once so the projection is populated before rename, matching
    // the realistic case where the projection already exists.
    let (_o, e, c) = run(&["--root", &repo.path().to_string_lossy(), "article", "index", "--project", "alpha"]);
    assert_eq!(c, 0, "stderr: {e}");

    // Rename the article.
    let (_stdout, stderr, code) = run(&[
        "--root",
        &repo.path().to_string_lossy(),
        "article",
        "rename",
        "docs/old-name.md",
        "new-name",
        "--project",
        "alpha",
    ]);
    assert_eq!(code, 0, "rename should succeed: stderr={stderr}");

    // Files physically renamed on disk.
    assert!(project.join("prompts/new-name.md").exists(), "prompt file should be renamed");
    assert!(project.join("thinking/new-name.md").exists(), "thinking file should be renamed");
    assert!(!project.join("prompts/old-name.md").exists());
    assert!(!project.join("thinking/old-name.md").exists());

    // Prompt frontmatter binding rewritten to the new article path.
    let prompt_content = std::fs::read_to_string(project.join("prompts/new-name.md")).unwrap();
    assert!(prompt_content.contains("article: docs/new-name.md"), "{prompt_content}");

    // Projections immediately reflect the rename — no re-index required.
    let (show_stdout, show_stderr, show_code) = run(&[
        "--root",
        &repo.path().to_string_lossy(),
        "--output",
        "json",
        "article",
        "show",
        "docs/new-name.md",
        "--project",
        "alpha",
    ]);
    assert_eq!(show_code, 0, "stderr: {show_stderr}");
    let parsed: serde_json::Value = serde_json::from_str(&show_stdout).unwrap();
    assert_eq!(parsed["data"]["prompt"]["path"], "prompts/new-name.md");
    assert_eq!(parsed["data"]["prompt"]["binding_status"], "bound");
    assert_eq!(parsed["data"]["thinking"]["path"], "thinking/new-name.md");

    // The stale old-name projection entry should be pruned — a bare `mf
    // article list` no longer shows it bound to anything.
    let (list_stdout, list_stderr, list_code) =
        run(&["--root", &repo.path().to_string_lossy(), "--output", "json", "article", "list", "--project", "alpha"]);
    assert_eq!(list_code, 0, "stderr: {list_stderr}");
    let parsed: serde_json::Value = serde_json::from_str(&list_stdout).unwrap();
    let articles = parsed["data"]["articles"].as_array().unwrap();
    assert!(
        !articles.iter().any(|a| a["prompt"]["path"] == "prompts/old-name.md"),
        "stale old-name projection entry should be pruned: {list_stdout}"
    );

    // No spurious lint findings.
    let (lint_stdout, lint_stderr, lint_code) =
        run(&["--root", &repo.path().to_string_lossy(), "--output", "json", "project", "lint", "--project", "alpha"]);
    let parsed: serde_json::Value = serde_json::from_str(&lint_stdout).unwrap();
    let issues = parsed["data"]["issues"].as_array().unwrap();
    let our_kinds = ["orphan_prompt", "duplicate_binding", "missing_thinking"];
    assert!(
        issues.iter().all(|i| !our_kinds.contains(&i["kind"].as_str().unwrap_or(""))),
        "no spurious prompt/thinking findings after rename: {lint_stdout}"
    );
    assert_eq!(lint_code, 0, "stderr: {lint_stderr}");
}
