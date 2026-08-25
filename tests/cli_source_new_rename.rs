use assert_cmd::Command;
use tempfile::TempDir;

mod common;

fn setup() -> (common::TempDir, TempDir, std::path::PathBuf) {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");

    let source_dir = TempDir::new().unwrap();
    let source = source_dir.path().join("paper.pdf");
    std::fs::write(&source, b"fake pdf content").unwrap();

    (repo, source_dir, source)
}

fn mf(repo: &common::TempDir) -> Command {
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.args(["--root", repo.path().to_str().unwrap(), "--project", "alpha"]);
    cmd
}

#[test]
fn source_new_copies_file_and_indexes_entry() {
    let (repo, _source_dir, source) = setup();

    let output = mf(&repo).args(["source", "new", source.to_str().unwrap()]).output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert!(String::from_utf8_lossy(&output.stderr).is_empty(), "new form should not warn");

    let project = repo.path().join("alpha");
    assert!(project.join("sources/pdf/paper.pdf").exists(), "source file should be copied");

    let index = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index.contains("paper"), "index should contain paper entry: {index}");
    assert!(index.contains("pdf"), "index should contain pdf kind: {index}");
}

// ── Spec 074 #32: actionable auto-naming collision error ─────────────────────

/// T015: registering a second same-stem file (no `-n`) fails with a usage error
/// naming the taken source AND suggesting a concrete unique `-n` value derived
/// from the path segment under the sources root (`dima-0731`).
#[test]
fn register_only_auto_named_collision_is_actionable() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");

    let yuque = project.join("sources/yuque/2026-07/0731.md");
    std::fs::create_dir_all(yuque.parent().unwrap()).unwrap();
    std::fs::write(&yuque, "first\n").unwrap();
    let output =
        mf(&repo).args(["source", "new", "sources/yuque/2026-07/0731.md", "--register-only"]).output().unwrap();
    assert!(output.status.success(), "first register should succeed: {}", String::from_utf8_lossy(&output.stderr));

    let dima = project.join("sources/dima/2026-07/0731.md");
    std::fs::create_dir_all(dima.parent().unwrap()).unwrap();
    std::fs::write(&dima, "second\n").unwrap();
    let output = mf(&repo).args(["source", "new", "sources/dima/2026-07/0731.md", "--register-only"]).output().unwrap();

    assert_eq!(output.status.code(), Some(2), "collision must be a usage error (exit 2)");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("0731"), "error must name the taken source: {stderr}");
    assert!(stderr.contains("already registered"), "error must say already registered: {stderr}");
    assert!(stderr.contains("-n dima-0731"), "error must suggest a concrete -n value: {stderr}");
}

/// FR-008: an explicit `-n` that collides also fails with the actionable
/// duplicate-name error (no auto-rename); a non-colliding explicit `-n` succeeds.
#[test]
fn explicit_name_collision_is_actionable_but_success_for_unique() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");

    let first = project.join("sources/yuque/2026-07/0731.md");
    std::fs::create_dir_all(first.parent().unwrap()).unwrap();
    std::fs::write(&first, "first\n").unwrap();
    let output =
        mf(&repo).args(["source", "new", "sources/yuque/2026-07/0731.md", "--register-only"]).output().unwrap();
    assert!(output.status.success(), "first register should succeed: {}", String::from_utf8_lossy(&output.stderr));

    // Explicit -n that collides → same actionable error naming the taken source.
    let dima = project.join("sources/dima/2026-07/0731.md");
    std::fs::create_dir_all(dima.parent().unwrap()).unwrap();
    std::fs::write(&dima, "second\n").unwrap();
    let output = mf(&repo)
        .args(["source", "new", "sources/dima/2026-07/0731.md", "--register-only", "--name", "0731"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2), "explicit collision must exit 2");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("0731"), "explicit collision must name the taken source: {stderr}");

    // A unique explicit -n succeeds (used verbatim, unchanged).
    let output = mf(&repo)
        .args(["source", "new", "sources/dima/2026-07/0731.md", "--register-only", "--name", "dima-0731"])
        .output()
        .unwrap();
    assert!(output.status.success(), "unique explicit -n should succeed: {}", String::from_utf8_lossy(&output.stderr));
    let index = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index.contains("dima-0731"), "explicit name used verbatim: {index}");
}

// ---------------------------------------------------------------------------
// Spec 075 US6: the Lance backend's registration path used to report the
// generic file-conflict error (`refusing to overwrite existing file`, hint
// `--force`) instead of the actionable naming error — and that hint is a
// dead end under `--register-only`, which rejects `--force` outright.
// ---------------------------------------------------------------------------

mod lance_backend_collision {
    use crate::common::embedding_provider::{provider_repo, run};

    /// T084/FR-033: an auto-derived name collision on the Lance backend path
    /// reports `already registered` and suggests `-n <parent>-<stem>` — the
    /// legacy-backend case above already covers the other (non-Lance) path.
    #[test]
    fn auto_derived_collision_on_lance_backend_is_actionable() {
        let repo = provider_repo();
        let project = repo.path().join("projects/alpha");
        // provider_repo() registers "notes" from sources/file/notes.md; a
        // second same-stem file directly under sources/ (one segment deep,
        // matching `suggest_unique_name`'s `<segment>-<stem>` derivation)
        // collides.
        std::fs::create_dir_all(project.join("sources/dima")).unwrap();
        std::fs::write(project.join("sources/dima/notes.md"), "second\n").unwrap();

        let (stdout, stderr, code) =
            run(&repo, &["source", "new", "sources/dima/notes.md", "--project", "alpha", "--register-only"], &[]);
        assert_ne!(code, 0, "collision must fail\nstdout:\n{stdout}\nstderr:\n{stderr}");
        assert!(stderr.contains("already registered"), "must say already registered: {stderr}");
        assert!(stderr.contains("notes"), "must name the taken source: {stderr}");
        assert!(stderr.contains("-n dima-notes"), "must suggest a concrete -n value from the path segment: {stderr}");
        assert!(!stderr.contains("--force"), "must never suggest --force, a dead end under --register-only: {stderr}");
    }

    /// T085/FR-033/FR-034: an explicit-name collision names the taken name
    /// without inventing a suggestion, and no hint names `--force` under
    /// `--register-only`.
    #[test]
    fn explicit_name_collision_on_lance_backend_names_taken_name_without_suggestion() {
        let repo = provider_repo();
        let project = repo.path().join("projects/alpha");
        std::fs::write(project.join("sources/file/other.md"), "second\n").unwrap();

        let (stdout, stderr, code) = run(
            &repo,
            &["source", "new", "sources/file/other.md", "--project", "alpha", "--register-only", "--name", "notes"],
            &[],
        );
        assert_ne!(code, 0, "collision must fail\nstdout:\n{stdout}\nstderr:\n{stderr}");
        assert!(stderr.contains("notes") && stderr.contains("already registered"), "{stderr}");
        assert!(!stderr.contains("-n "), "an explicit-name collision must not invent a suggestion: {stderr}");
        assert!(!stderr.contains("--force"), "must not suggest --force under --register-only: {stderr}");
    }

    /// T086/FR-033: the register-only and full-copy registration paths on the
    /// Lance backend produce an identical message for the equivalent
    /// collision (both route through the same `add_registration` branch).
    #[test]
    fn both_lance_registration_paths_produce_identical_collision_message() {
        let repo = provider_repo();
        let project = repo.path().join("projects/alpha");
        std::fs::create_dir_all(project.join("sources/dima")).unwrap();
        std::fs::write(project.join("sources/dima/notes.md"), "second\n").unwrap();

        let (_, stderr_register_only, code_a) =
            run(&repo, &["source", "new", "sources/dima/notes.md", "--project", "alpha", "--register-only"], &[]);
        assert_ne!(code_a, 0);

        // A different kind (.pdf, so its copy destination sources/pdf/notes.pdf
        // does not collide on disk with the already-registered
        // sources/file/notes.md) whose derived name still collides in the store.
        let external = repo.path().join("notes.pdf");
        std::fs::write(&external, "copied variant\n").unwrap();
        let (_, stderr_copy, code_b) =
            run(&repo, &["source", "new", external.to_str().unwrap(), "--project", "alpha"], &[]);
        assert_ne!(code_b, 0);

        // Both paths hit the same `add_registration` collision branch, so
        // they share the same wording template — "already registered" plus
        // a concrete `-n` suggestion — even though the suggested value
        // differs because the two files were placed in different segments.
        for stderr in [&stderr_register_only, &stderr_copy] {
            assert!(stderr.contains("source name 'notes' is already registered"), "{stderr}");
            assert!(stderr.contains("try -n "), "{stderr}");
        }
    }
}
