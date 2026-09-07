//! Spec 079 T066: an end-to-end walk across US1/US2/US3/US4/US5 in one flow —
//! create a directory article, target it with a **bare slug** through `block
//! new` (US3, #46), write a private callout with a bare blank line inside it
//! (US2, #49), build (zero leakage + exactly one warning), then rebuild the
//! index (US4/US5, #53: title stays slug-derived, existing timestamps
//! survive the rebuild).

use std::fs;

use crate::datasets::Dataset;
use crate::helpers::run_in;

#[test]
fn directory_article_bare_slug_private_callout_and_index_rebuild_flow() {
    let ds = Dataset::empty().with_standard_project("demo");
    let project = ds.root().join("projects/demo");

    // 1. Create a directory article with a title deliberately different from
    //    its slug, so a later bare-slug match can only work via US3's
    //    resolver, not by accident matching on title.
    let (_, stderr, code) = run_in(&project, &["article", "new", "Monthly Report", "--slug", "2026-09-monthly"]);
    assert_eq!(code, 0, "article new failed: {stderr}");
    assert!(project.join("docs/2026-09-monthly").is_dir());

    // 2. Target it with a **bare slug** through `block new` (US3, #46) — this
    //    is the exact operation that used to report "not found".
    let (_, stderr, code) = run_in(&project, &["article", "block", "new", "2026-09-monthly", "progress"]);
    assert_eq!(code, 0, "block new by bare slug failed: {stderr}");
    let progress_file = project.join("docs/2026-09-monthly/02-progress.md");
    assert!(progress_file.exists(), "block new should have created 02-progress.md");

    // 3. Write a private callout with a bare blank line inside it, followed
    //    by more quoted content (US2, #49).
    fs::write(&progress_file, "## Progress\n\n> [!mf-private]\n> front half\n\n> back half\n\nPublic tail.\n").unwrap();

    // 4. Build: zero leakage, exactly one warning naming the file/line.
    let (_, stderr, code) = run_in(&project, &["build", "2026-09-monthly"]);
    assert_eq!(code, 0, "build failed: {stderr}");
    let output_content = fs::read_to_string(project.join("outputs/2026-09-monthly.md")).unwrap();
    assert!(!output_content.contains("front half"), "private content must not leak: {output_content}");
    assert!(!output_content.contains("back half"), "content after the bare blank line must not leak: {output_content}");
    assert!(output_content.contains("Public tail."), "public content must survive: {output_content}");
    let warn_lines: Vec<&str> = stderr.lines().filter(|l| l.contains("WARN:")).collect();
    assert_eq!(warn_lines.len(), 1, "exactly one warning, stderr: {stderr}");

    // 5. Rebuild the index once to establish a baseline (this article was
    //    already registered by `article new` above, at whatever title it was
    //    given then — US5's title-from-slug derivation applies to entries
    //    freshly *discovered* by the docs scan, not to already-registered
    //    ones; a separate, focused unit test covers that discovery path).
    let (_, stderr, code) = run_in(&project, &["article", "index"]);
    assert_eq!(code, 0, "article index failed: {stderr}");
    let index_after_first_rebuild = fs::read_to_string(project.join("mind-index.yaml")).unwrap();

    // 6. Rewrite one block's heading (the article's would-be "H1") and
    //    rebuild (US5, #53): title must not follow the heading change.
    fs::write(&progress_file, "## A Completely Different Heading\n\nStill public.\n").unwrap();
    let (_, stderr, code) = run_in(&project, &["article", "index"]);
    assert_eq!(code, 0, "second article index failed: {stderr}");
    let index_after_heading_rewrite = fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert_eq!(
        index_after_first_rebuild, index_after_heading_rewrite,
        "title (and every other stable field) must survive a heading rewrite: no diff expected"
    );

    // 7. Rebuild again with no disk changes at all (US4, #53): existing
    //    timestamps must not be rewritten — the index diff must be empty.
    let (_, stderr, code) = run_in(&project, &["article", "index"]);
    assert_eq!(code, 0, "third article index failed: {stderr}");
    let index_after_third_rebuild = fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert_eq!(
        index_after_heading_rewrite, index_after_third_rebuild,
        "a rebuild with no disk changes must produce a byte-identical index"
    );
}
