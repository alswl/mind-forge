//! E2E: Bug #22 — `mf build` from a deep, symlinked (worktree-like) checkout
//! must produce the same valid relative image references as building from
//! the plain repository root, never a malformed mixed absolute/relative path.

use std::fs;

use crate::helpers::*;

fn scaffold_project(root: &std::path::Path) {
    fs::write(root.join("minds.yaml"), "schema: '1'\nprojects:\n  - projects/2026-blogs\n").unwrap();
    let project = root.join("projects/2026-blogs");
    fs::create_dir_all(project.join("docs/my-article")).unwrap();
    fs::create_dir_all(project.join("assets")).unwrap();
    fs::write(project.join("mind.yaml"), "schema: '1'\n").unwrap();
    fs::write(
        project.join("docs/my-article/01-opening.md"),
        "# Opening\n\n![hero](../../assets/pic.png)\n\n[ref]: ../../assets/pic.png\n\n<img src=\"../../assets/pic.png\">\n",
    )
    .unwrap();
}

/// E2E: building via a deeply nested symlinked worktree-like path produces
/// byte-identical, valid relative image references as building from the
/// plain repository root.
#[cfg(unix)]
#[test]
fn build_from_symlinked_worktree_matches_plain_root_build() {
    use std::os::unix::fs::symlink;

    // Plain-root build.
    let plain_root = TempDir::new().unwrap();
    scaffold_project(plain_root.path());
    let plain_project = plain_root.path().join("projects/2026-blogs");
    let (_, stderr, code) =
        run_in(&plain_project, &["build", "@projects/2026-blogs/docs/my-article/", "--out", "outputs/my-article.md"]);
    assert_eq!(code, 0, "plain-root build must succeed: {stderr}");
    let plain_content = fs::read_to_string(plain_project.join("outputs/my-article.md")).unwrap();

    // Deep, symlinked "worktree" build: nest the real repo several levels
    // deep, then reach it through a symlink — mirroring
    // `.claude/worktrees/<name>` style checkouts.
    let real_deep_root = TempDir::new().unwrap();
    let nested = real_deep_root.path().join("a/b/c/worktrees/feature-branch");
    fs::create_dir_all(&nested).unwrap();
    scaffold_project(&nested);
    let link_parent = TempDir::new().unwrap();
    let symlinked_root = link_parent.path().join("worktree-link");
    symlink(&nested, &symlinked_root).unwrap();
    let symlinked_project = symlinked_root.join("projects/2026-blogs");

    let (_, stderr, code) = run_in(
        &symlinked_project,
        &["build", "@projects/2026-blogs/docs/my-article/", "--out", "outputs/my-article.md"],
    );
    assert_eq!(code, 0, "symlinked worktree build must succeed: {stderr}");
    assert!(!stderr.contains("WARN:"), "no path warnings expected for a resolvable reference: {stderr}");

    // Read back through the real (non-symlinked) path.
    let real_project = nested.join("projects/2026-blogs");
    let worktree_content = fs::read_to_string(real_project.join("outputs/my-article.md")).unwrap();

    assert_eq!(
        plain_content, worktree_content,
        "image references must be identical between plain-root and symlinked worktree builds"
    );
    assert!(
        worktree_content.contains("![hero](../assets/pic.png)"),
        "must contain a valid relative image path: {worktree_content}"
    );
    assert!(
        worktree_content.contains("[ref]: ../assets/pic.png"),
        "must contain a valid relative reference definition: {worktree_content}"
    );
    assert!(
        worktree_content.contains(r#"<img src="../assets/pic.png">"#),
        "must contain a valid relative HTML img src: {worktree_content}"
    );
    for malformed in ["////", "..//", "///"] {
        assert!(!worktree_content.contains(malformed), "must never contain {malformed:?}: {worktree_content}");
    }
}
