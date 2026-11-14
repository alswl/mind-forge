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
    assert!(stderr.contains("--name dima-0731"), "error must suggest a concrete --name value: {stderr}");
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
// Spec 081 US1: force replacement preserves timestamps, tags and extra fields.
// ---------------------------------------------------------------------------

/// Rewrite every `added_at:`/`updated_at:` value in a projection so a later
/// reset is unambiguous. The CLI has no flag for these, so the projection is
/// the only way to seed them.
fn backdate_timestamps(index: &str, stamp: &str) -> String {
    let mut out = index
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with("added_at:") {
                format!("    added_at: {stamp}")
            } else if trimmed.starts_with("updated_at:") {
                format!("    updated_at: {stamp}")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    out.push('\n');
    out
}

/// Spec 075 FR-011 + spec 081 FR-002: replacing a same-name registration keeps
/// the logical record's creation time, its tags, and any field the system does
/// not interpret; only `updated_at` moves.
#[test]
fn register_only_force_preserves_added_at_tags_and_extras_on_legacy_backend() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    let index_path = project.join("mind-index.yaml");

    let first = project.join("sources/yuque/2026-07/0731.md");
    std::fs::create_dir_all(first.parent().unwrap()).unwrap();
    std::fs::write(&first, "first\n").unwrap();
    let output = mf(&repo)
        .args(["source", "new", "sources/yuque/2026-07/0731.md", "--register-only", "--name", "shared"])
        .output()
        .unwrap();
    assert!(output.status.success(), "first register should succeed: {}", String::from_utf8_lossy(&output.stderr));

    let seeded = std::fs::read_to_string(&index_path)
        .unwrap()
        .replace("    tags: []\n", "    tags:\n      - alpha\n      - beta\n")
        .replace("    name: shared\n", "    name: shared\n    provenance_note: keep-me-verbatim\n");
    std::fs::write(&index_path, backdate_timestamps(&seeded, "2020-01-01T00:00:00Z")).unwrap();

    let second = project.join("sources/dima/2026-07/0731.md");
    std::fs::create_dir_all(second.parent().unwrap()).unwrap();
    std::fs::write(&second, "second\n").unwrap();
    let output = mf(&repo)
        .args(["source", "new", "sources/dima/2026-07/0731.md", "--register-only", "--name", "shared", "--force"])
        .output()
        .unwrap();
    assert!(output.status.success(), "--force replacement must succeed: {}", String::from_utf8_lossy(&output.stderr));

    let index = std::fs::read_to_string(&index_path).unwrap();
    assert!(index.contains("dima/2026-07/0731.md"), "replacement must point at the second file: {index}");
    assert!(index.contains("added_at: 2020-01-01T00:00:00Z"), "added_at must survive the replacement: {index}");
    assert!(index.contains("keep-me-verbatim"), "FR-011: passthrough fields must round-trip: {index}");
    assert!(index.contains("- alpha") && index.contains("- beta"), "tags must survive the replacement: {index}");
    assert!(!index.contains("updated_at: 2020-01-01T00:00:00Z"), "updated_at must be refreshed: {index}");
}
