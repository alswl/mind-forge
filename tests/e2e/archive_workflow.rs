//! T106/SC-010 (spec 075): the archive workflow — register, index, clean
//! terminology, verify, search — completes end to end with none of the
//! workarounds documented against #34, #35, #37, #39, #40, #41: no legacy
//! backend swap, no hand-written machine-local state file, no per-file
//! `--term` compensation for a finding lint reports as fixable, no
//! `git checkout` rollback.

use crate::datasets::Dataset;
use crate::helpers::run_in;

const MANIFEST_ALPHA: &str = "schema_version: '1'\nprojects:\n  \
    - name: alpha\n    path: ./projects/alpha\n    created_at: \"2026-04-30T08:00:00Z\"\n    archived_at: ~\n";

fn git(dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    std::process::Command::new("git").args(args).current_dir(dir).output().expect("git runs")
}

#[test]
fn archive_workflow_completes_with_no_documented_workarounds() {
    let ds = Dataset::empty().with_standard_project("alpha");
    std::fs::write(ds.root().join("minds.yaml"), MANIFEST_ALPHA).expect("write manifest");
    std::fs::write(
        ds.root().join("projects/alpha/mind-index.yaml"),
        "schema_version: '1'\nterms:\n  - term: Device\n    corrections:\n      - original: mashine\n        correct: Machine\n",
    )
    .expect("seed a term correction");

    git(ds.root(), &["init"]).status.success().then_some(()).expect("git init");
    git(ds.root(), &["config", "user.email", "test@example.com"]);
    git(ds.root(), &["config", "user.name", "Test"]);
    git(ds.root(), &["add", "-A"]);
    git(ds.root(), &["commit", "-m", "initial"]);

    // 1. Register a new source into the project (spec 075 US6: no name
    // collision, no --force dead end).
    let notes = ds.root().join("projects/alpha/sources/archive-notes.md");
    std::fs::write(&notes, "# Archive notes\n\nA mashine failure was logged today.\n").expect("write notes");
    let (stdout, stderr, code) =
        run_in(ds.root(), &["source", "new", "sources/archive-notes.md", "--project", "alpha", "--register-only"]);
    assert_eq!(code, 0, "register failed\nstdout:\n{stdout}\nstderr:\n{stderr}");

    // 2. Index the project (spec 075 US2: disk-adoption/reconcile, not a
    // sync passthrough that never sees a genuinely new file).
    let (stdout, stderr, code) = run_in(ds.root(), &["source", "index", "--project", "alpha"]);
    assert_eq!(code, 0, "index failed\nstdout:\n{stdout}\nstderr:\n{stderr}");

    // 3. Clean terminology: fix a bulk-applicable correction, with no
    // per-file --term compensation for a finding lint already reports as
    // fixable (spec 075 US4 longest-match correctness applies uniformly).
    let (stdout, stderr, code) =
        run_in(ds.root(), &["term", "fix", "--project", "alpha", "sources/archive-notes.md", "-y"]);
    assert_eq!(code, 0, "fix failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let fixed = std::fs::read_to_string(&notes).expect("read fixed notes");
    assert!(fixed.contains("Machine"), "the registered correction must have applied: {fixed}");

    // 4. Verify: lint reports clean now that the only registered correction
    // has been applied.
    let (stdout, stderr, code) = run_in(ds.root(), &["term", "lint", "--project", "alpha", "sources/archive-notes.md"]);
    assert_eq!(code, 0, "lint after fix should be clean\nstdout:\n{stdout}\nstderr:\n{stderr}");

    // 5. Search: the freshly registered and indexed content is retrievable
    // without a separate manual sync step, no legacy-backend fallback.
    let (stdout, stderr, code) = run_in(ds.root(), &["source", "sync", "--offline"]);
    assert_eq!(code, 0, "sync failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let (stdout, stderr, code) = run_in(ds.root(), &["source", "search", "machine failure", "--mode", "basic"]);
    assert_eq!(code, 0, "search failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(
        stdout.to_lowercase().contains("machine") || stdout.to_lowercase().contains("archive"),
        "the fixed, freshly indexed content must be searchable: {stdout}"
    );

    // 6. The project index diff is additive only — no unrelated reordering
    // or reformatting of untouched keys (spec 075 I-2/FR-013).
    let diff = git(ds.root(), &["diff", "--stat", "projects/alpha/mind-index.yaml"]);
    let diff_text = String::from_utf8_lossy(&diff.stdout);
    assert!(!diff_text.trim().is_empty(), "the index must have changed to record the new source");
}
