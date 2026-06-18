use crate::datasets::Dataset;
use crate::helpers::*;

// ---------------------------------------------------------------------------
// Global term lifecycle E2E
// ---------------------------------------------------------------------------

/// E2E: Complete global term workflow — new → add → lint → rename → remove
#[test]
fn global_term_full_lifecycle() {
    let ds = Dataset::empty();
    let root = ds.root();

    // Create a markdown file to lint against
    let doc = "docs/test.md";
    let doc_dir = root.join("docs");
    std::fs::create_dir_all(&doc_dir).unwrap();
    std::fs::write(root.join(doc), "# Test\n\nWe use k8s for orchestration.\n").unwrap();

    // 1. Create global term
    let (stdout, _stderr, code) =
        run_in(root, &["term", "new", "Kubernetes", "--definition", "Container orchestration"]);
    assert_eq!(code, 0, "term new should succeed, got: {stdout} {_stderr}");
    assert!(stdout.contains("created term"), "stdout: {stdout}");

    // 2. Add correction (alias) for the term
    let (stdout, _stderr, code) = run_in(root, &["term", "add", "--term", "Kubernetes", "--alias", "k8s"]);
    assert_eq!(code, 0, "term add should succeed, got: {stdout} {_stderr}");
    assert!(stdout.contains("added alias") || stdout.contains("created term"), "stdout: {stdout}");

    // 3. Lint the doc (dry-run)
    let (stdout, _stderr, code) = run_in(root, &["term", "lint", doc, "--fix", "--dry-run"]);
    assert_eq!(code, 1, "lint should find k8s, got: {stdout} {_stderr}");
    assert!(stdout.contains("k8s") || stdout.contains("findings"), "stdout: {stdout}");

    // 4. Rename the term
    let (stdout, _stderr, code) = run_in(root, &["term", "rename", "Kubernetes", "K8s", "--keep-alias"]);
    assert_eq!(code, 0, "term rename should succeed, got: {stdout} {_stderr}");
    assert!(stdout.contains("renamed"), "stdout: {stdout}");

    // 5. Remove the term
    let (stdout, _stderr, code) = run_in(root, &["term", "remove", "K8s", "--force"]);
    assert_eq!(code, 0, "term remove should succeed, got: {stdout} {_stderr}");
    assert!(stdout.contains("removed"), "stdout: {stdout}");
}

/// E2E: Global term lint works from repo root without --project
#[test]
fn global_term_lint_no_project() {
    let ds = Dataset::empty();
    let root = ds.root();

    // Create a doc with a known misrecognition
    let doc_dir = root.join("docs");
    std::fs::create_dir_all(&doc_dir).unwrap();
    std::fs::write(root.join("docs/test.md"), "We use kube for orchestration.\n").unwrap();

    // Register a global term with correction
    run_in(root, &["term", "new", "Kubernetes", "--definition", "Orchestration"]);
    run_in(root, &["term", "add", "--term", "Kubernetes", "--alias", "kube"]);

    // Lint should find "kube" without --project
    let (stdout, _stderr, code) = run_in(root, &["term", "lint", "docs/test.md"]);
    assert_eq!(code, 1, "lint should find kube without --project, got: {stdout} {_stderr}");
    assert!(stdout.contains("kube") && stdout.contains("Kubernetes"), "stdout: {stdout}");
}

/// E2E: Global term remove works from repo root without --project
#[test]
fn global_term_remove_no_project() {
    let ds = Dataset::empty();
    let root = ds.root();

    run_in(root, &["term", "new", "test-term", "--definition", "A test term"]);

    let (stdout, _stderr, code) = run_in(root, &["term", "remove", "test-term", "--force"]);
    assert_eq!(code, 0, "term remove without --project should succeed, got: {stdout} {_stderr}");
    assert!(stdout.contains("removed"), "stdout: {stdout}");

    // Verify it's gone
    let (stdout, stderr, code) = run_in(root, &["term", "show", "test-term"]);
    assert_eq!(code, 2, "term show should fail after removal, got: {stdout} {stderr}");
}

/// E2E: Global term rename works from repo root without --project
#[test]
fn global_term_rename_no_project() {
    let ds = Dataset::empty();
    let root = ds.root();

    run_in(root, &["term", "new", "old-name", "--definition", "Will be renamed"]);

    let (stdout, _stderr, code) = run_in(root, &["term", "rename", "old-name", "new-name", "--force"]);
    assert_eq!(code, 0, "term rename without --project should succeed, got: {stdout} {_stderr}");
    assert!(stdout.contains("renamed"), "stdout: {stdout}");

    // Verify old name is gone
    let (_stdout, _stderr, code) = run_in(root, &["term", "show", "old-name"]);
    assert_eq!(code, 2, "old name should not exist after rename");
}

/// E2E: Short flag -p works for project-scoped operations
#[test]
fn short_p_flag_for_project_scoped() {
    let ds = Dataset::empty().with_project("demo");
    let root = ds.root();

    run_in(root, &["-p", "demo", "term", "new", "project-term", "--definition", "Scoped to project"]);

    let (stdout, _stderr, code) = run_in(root, &["-p", "demo", "term", "list"]);
    assert_eq!(code, 0, "-p demo term list should succeed, got: {stdout} {_stderr}");
    assert!(stdout.contains("project-term"), "should find project-term, got: {stdout}");
}

/// E2E: rename --keep-alias preserves old name
#[test]
fn global_term_rename_keep_alias() {
    let ds = Dataset::empty();
    let root = ds.root();

    run_in(root, &["term", "new", "original", "--definition", "Original name"]);

    run_in(root, &["term", "rename", "original", "renamed", "--keep-alias", "--force"]);

    // The new name should work
    let (stdout, _stderr, code) = run_in(root, &["term", "show", "renamed"]);
    assert_eq!(code, 0, "should find renamed term, got: {stdout}");
}

/// E2E: rename duplicate target fails without --force
#[test]
fn global_term_rename_duplicate_fails_without_force() {
    let ds = Dataset::empty();
    let root = ds.root();

    run_in(root, &["term", "new", "term-a", "--definition", "First term"]);
    run_in(root, &["term", "new", "term-b", "--definition", "Second term"]);

    let (_stdout, stderr, code) = run_in(root, &["term", "rename", "term-a", "term-b"]);
    assert_eq!(code, 2, "rename to existing name without --force should fail, got: {stderr}");
    assert!(stderr.contains("already exists"), "stderr: {stderr}");
}

/// E2E: lint with no terms registered shows appropriate message
#[test]
fn global_term_lint_no_terms() {
    let ds = Dataset::empty();
    let root = ds.root();

    // Create a doc but don't register any terms
    let doc_dir = root.join("docs");
    std::fs::create_dir_all(&doc_dir).unwrap();
    std::fs::write(root.join("docs/test.md"), "hello world\n").unwrap();

    let (stdout, stderr, code) = run_in(root, &["term", "lint", "docs/test.md"]);
    assert_eq!(code, 0, "lint with no terms should succeed, got: {stderr}");
    assert!(stdout.contains("No terms") || stdout.contains("No term"), "stdout: {stdout}");
}

/// E2E: lint invalid path returns error
#[test]
fn global_term_lint_invalid_path() {
    let ds = Dataset::empty();
    let root = ds.root();

    // Register a term so we have something to lint with
    run_in(root, &["term", "new", "TestTerm", "--definition", "Test"]);

    let (_stdout, stderr, code) = run_in(root, &["term", "lint", "nonexistent/file.md"]);
    assert_eq!(code, 2, "lint with invalid path should fail, got: {stderr}");
    assert!(stderr.contains("file not found") || stderr.contains("error"), "stderr: {stderr}");
}
