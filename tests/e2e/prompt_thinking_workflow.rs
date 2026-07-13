use std::fs;

use serde_json::Value;

use crate::datasets::Dataset;
use crate::helpers::run_in;

/// End-to-end workflow from quickstart.md (spec 065): create an
/// article+prompt+thinking triple, reconcile both projections, confirm
/// `bound` status via `list`, then delete the article and confirm
/// `project lint` reports `orphan_prompt` with a non-zero exit code.
#[test]
fn prompt_thinking_reconcile_list_then_lint_detects_orphan() {
    let ds = Dataset::empty().with_standard_project("alpha");
    let project = ds.root().join("projects/alpha");

    fs::create_dir_all(project.join("prompts")).unwrap();
    fs::create_dir_all(project.join("thinking")).unwrap();
    fs::write(project.join("docs/my-post.md"), "# My Post\n\nContent.\n").unwrap();
    fs::write(
        project.join("prompts/my-post.md"),
        "---\narticle: docs/my-post.md\nmode: research\n---\n\nResearch brief.\n",
    )
    .unwrap();
    fs::write(project.join("thinking/my-post.md"), "## Comparisons\n\nWorking ledger.\n").unwrap();

    // Register the article itself first (mirrors a real workflow where the
    // article was created via `mf article new` / `mf article index`).
    let (_stdout, stderr, code) = run_in(ds.root(), &["article", "index", "--project", "alpha"]);
    assert_eq!(code, 0, "article index failed: {stderr}");

    // 1. Reconcile the prompt and thinking projections.
    let (stdout, stderr, code) = run_in(ds.root(), &["prompt", "index", "--project", "alpha"]);
    assert_eq!(code, 0, "prompt index failed: {stderr}");
    assert!(stdout.contains("+1") || stdout.contains("added"), "{stdout}");

    let (stdout, stderr, code) = run_in(ds.root(), &["thinking", "index", "--project", "alpha"]);
    assert_eq!(code, 0, "thinking index failed: {stderr}");
    assert!(stdout.contains("+1") || stdout.contains("added"), "{stdout}");

    // 2. `prompt list` shows the entry as bound.
    let (stdout, stderr, code) = run_in(ds.root(), &["--output", "json", "prompt", "list", "--project", "alpha"]);
    assert_eq!(code, 0, "prompt list failed: {stderr}");
    let envelope: Value = serde_json::from_str(&stdout).expect("prompt list stdout should be json");
    let prompts = envelope["data"]["prompts"].as_array().expect("data.prompts should be an array");
    let entry = prompts.iter().find(|p| p["path"] == "prompts/my-post.md").expect("prompt present in list");
    assert_eq!(entry["binding_status"], "bound", "{stdout}");
    assert_eq!(entry["article"], "docs/my-post.md");

    // A clean project lints with zero prompt/thinking findings.
    let (stdout, stderr, code) = run_in(ds.root(), &["--output", "json", "project", "lint", "--project", "alpha"]);
    assert_eq!(code, 0, "lint of clean project should pass: {stdout}\nstderr: {stderr}");

    // 3. Delete the article, leaving the prompt/thinking projections stale
    //    (as they'd be immediately after a manual `rm`, before re-indexing).
    fs::remove_file(project.join("docs/my-post.md")).unwrap();
    let (_stdout, stderr, code) = run_in(ds.root(), &["article", "index", "--project", "alpha"]);
    assert_eq!(code, 0, "article index (post-delete) failed: {stderr}");

    // 4. `project lint` now reports `orphan_prompt` and fails.
    let (stdout, stderr, code) = run_in(ds.root(), &["--output", "json", "project", "lint", "--project", "alpha"]);
    let envelope: Value = serde_json::from_str(&stdout).expect("project lint stdout should be json");
    let issues = envelope["data"]["issues"].as_array().expect("data.issues should be an array");
    let orphan = issues.iter().find(|i| i["kind"] == "orphan_prompt").expect("orphan_prompt finding present");
    assert_eq!(orphan["severity"], "error");
    assert_eq!(code, 1, "orphan_prompt must fail lint: stdout={stdout}\nstderr={stderr}");
}
