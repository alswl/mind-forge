use crate::datasets::Dataset;
use crate::helpers::*;

/// E2E: Cross-scope term lifecycle — create in project, add correction, move to global,
/// verify show/list, then remove.
#[test]
fn term_cross_scope_lifecycle() {
    let ds = Dataset::empty().with_project("alpha");
    let root = ds.root();

    // 1. Create a project-scoped term
    let (stdout, _, code) =
        run_in(root, &["-p", "alpha", "term", "new", "Kubernetes", "--definition", "Container orchestration"]);
    assert_eq!(code, 0, "term new: {stdout}");
    assert!(stdout.contains("created term"), "stdout: {stdout}");

    // 2. Add a correction via correction subcommand
    let (stdout, _, code) =
        run_in(root, &["-p", "alpha", "term", "correction", "add", "Kubernetes", "k8s", "Kubernetes"]);
    assert_eq!(code, 0, "correction add: {stdout}");
    assert!(stdout.contains("added correction"), "stdout: {stdout}");

    // 3. Verify correction is listed
    let (stdout, _, code) = run_in(root, &["-p", "alpha", "term", "correction", "list", "Kubernetes"]);
    assert_eq!(code, 0, "correction list: {stdout}");
    assert!(stdout.contains("k8s"), "correction should appear: {stdout}");

    // 4. Show confirms corrections present
    let (stdout, _, code) = run_in(root, &["-p", "alpha", "term", "show", "Kubernetes"]);
    assert_eq!(code, 0, "term show: {stdout}");
    assert!(stdout.contains("Kubernetes"), "term show: {stdout}");

    // 5. List confirms term is in project scope
    let (stdout, _, code) = run_in(root, &["-p", "alpha", "term", "list"]);
    assert_eq!(code, 0, "term list: {stdout}");
    assert!(stdout.contains("Kubernetes"), "term list: {stdout}");

    // 6. Move to global scope
    let (stdout, _, code) = run_in(root, &["-p", "alpha", "term", "move", "Kubernetes", "--to-global"]);
    assert_eq!(code, 0, "term move: {stdout}");

    // 7. Term no longer in project scope (--scope project to exclude global fallback)
    let (stdout, _, code) = run_in(root, &["-p", "alpha", "term", "list", "--scope", "project"]);
    assert_eq!(code, 0);
    assert!(!stdout.contains("Kubernetes"), "term should have left project: {stdout}");

    // 8. Term is now findable globally (via list without --project)
    let (stdout, _, code) = run_in(root, &["term", "list"]);
    assert_eq!(code, 0, "global term list: {stdout}");
    assert!(stdout.contains("Kubernetes"), "term should be in global scope: {stdout}");

    // 9. Correction survives the move
    let (stdout, _, code) = run_in(root, &["term", "correction", "list", "Kubernetes"]);
    assert_eq!(code, 0, "correction list after move: {stdout}");
    assert!(stdout.contains("k8s"), "correction should survive move: {stdout}");

    // 10. Remove from global scope
    let (stdout, _, code) = run_in(root, &["term", "remove", "Kubernetes", "--force"]);
    assert_eq!(code, 0, "term remove: {stdout}");
    assert!(stdout.contains("removed"), "stdout: {stdout}");

    // 11. Term is gone
    let (stdout, _, code) = run_in(root, &["term", "list"]);
    assert_eq!(code, 0);
    assert!(!stdout.contains("Kubernetes"), "term should be gone after remove: {stdout}");
}
