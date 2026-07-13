use std::fs;

use serde_json::Value;

use crate::datasets::Dataset;
use crate::helpers::run_in;

/// End-to-end workflow from quickstart.md (spec 065, Revision R1): create an
/// article+prompt+thinking triple, reconcile all three projections via a
/// single `mf article index`, confirm `bound` status via `article show`,
/// then delete the article and confirm `project lint` reports
/// `orphan_prompt` with a non-zero exit code.
///
/// Prompt/thinking binding health is surfaced through `mf article`, not
/// standalone `mf prompt`/`mf thinking` command groups (removed before merge
/// — see spec.md Revision R1).
#[test]
fn prompt_thinking_reconcile_show_then_lint_detects_orphan() {
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

    // 1. A single `article index` reconciles articles, prompts, and thinking together.
    let (stdout, stderr, code) = run_in(ds.root(), &["--output", "json", "article", "index", "--project", "alpha"]);
    assert_eq!(code, 0, "article index failed: {stderr}");
    let envelope: Value = serde_json::from_str(&stdout).expect("article index stdout should be json");
    assert_eq!(envelope["data"]["prompts"]["added"].as_array().unwrap().len(), 1, "{stdout}");
    assert_eq!(envelope["data"]["thinking"]["added"].as_array().unwrap().len(), 1, "{stdout}");

    // 2. `article show` reports the binding as bound.
    let (stdout, stderr, code) =
        run_in(ds.root(), &["--output", "json", "article", "show", "docs/my-post.md", "--project", "alpha"]);
    assert_eq!(code, 0, "article show failed: {stderr}");
    let envelope: Value = serde_json::from_str(&stdout).expect("article show stdout should be json");
    assert_eq!(envelope["data"]["prompt"]["binding_status"], "bound", "{stdout}");
    assert_eq!(envelope["data"]["prompt"]["path"], "prompts/my-post.md");
    assert_eq!(envelope["data"]["thinking"]["path"], "thinking/my-post.md");

    // A clean project lints with zero prompt/thinking findings.
    let (stdout, stderr, code) = run_in(ds.root(), &["--output", "json", "project", "lint", "--project", "alpha"]);
    assert_eq!(code, 0, "lint of clean project should pass: {stdout}\nstderr: {stderr}");

    // 3. Delete the article, leaving the prompt/thinking files orphaned; re-index.
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
