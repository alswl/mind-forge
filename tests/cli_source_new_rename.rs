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
