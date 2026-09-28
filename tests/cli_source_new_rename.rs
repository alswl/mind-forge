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
// Spec 075 US6: the Lance backend's registration path used to report the
// generic file-conflict error (`refusing to overwrite existing file`, hint
// `--force`) instead of the actionable naming error. Both backends now accept
// `--register-only --force` and replace the existing same-name registration
// in place (spec 081 US1); the collision error below is only reached when
// `--force` is absent.
// ---------------------------------------------------------------------------

mod lance_backend_collision {
    use crate::common::embedding_provider::{provider_repo, run};

    /// Value of the `added_at` line belonging to the entry named `name` in a
    /// `mind-index.yaml` projection, or `None` when the entry has no timestamp.
    fn added_at_for(index: &str, name: &str) -> Option<String> {
        let mut lines = index.lines().skip_while(|line| line.trim() != format!("name: {name}"));
        lines.next()?;
        lines
            .take_while(|line| !line.trim().starts_with("name: "))
            .find_map(|line| line.trim().strip_prefix("added_at: ").map(|v| v.to_string()))
    }

    /// T084/FR-033: an auto-derived name collision on the Lance backend path
    /// reports `already registered` and suggests `--name <parent>-<stem>` — the
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
        assert!(
            stderr.contains("--name dima-notes"),
            "must suggest a concrete --name value from the path segment: {stderr}"
        );
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
        assert!(!stderr.contains("try --name"), "an explicit-name collision must not invent a suggestion: {stderr}");
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
        // a concrete `--name` suggestion — even though the suggested value
        // differs because the two files were placed in different segments.
        // The hint must name the long flag: `-n` means `--dry-run` since #51,
        // so a `-n <value>` suggestion would fail if the user followed it.
        for stderr in [&stderr_register_only, &stderr_copy] {
            assert!(stderr.contains("source name 'notes' is already registered"), "{stderr}");
            assert!(stderr.contains("try --name "), "{stderr}");
            assert!(!stderr.contains("try -n "), "hint must not suggest the retired short flag: {stderr}");
        }
    }

    /// Spec 081 US1/FR-001/FR-002/FR-003/SC-001: `--register-only --force` on
    /// the Lance backend replaces a same-name registration in one command —
    /// without `--force` the collision still refuses with a `--name` hint
    /// (never `-n`, which is `--dry-run`'s short flag); with `--force` the
    /// second file's registration replaces the first's, leaving exactly one
    /// row for that name and a projection re-export with no drift.
    #[test]
    fn register_only_force_replaces_registration_on_lance_backend() {
        let repo = provider_repo();
        let project = repo.path().join("projects/alpha");
        std::fs::create_dir_all(project.join("sources/dima")).unwrap();
        std::fs::write(project.join("sources/dima/other.md"), "second\n").unwrap();

        // Without --force: refused, hint names --name, never -n.
        let (_, stderr, code) = run(
            &repo,
            &["source", "new", "sources/dima/other.md", "--project", "alpha", "--register-only", "--name", "notes"],
            &[],
        );
        assert_ne!(code, 0, "collision without --force must refuse");
        assert!(stderr.contains("source name 'notes' is already registered"), "{stderr}");
        assert!(!stderr.contains("-n "), "hint must never suggest the retired short flag: {stderr}");

        // With --force: succeeds, replaces the registration.
        let (stdout, stderr, code) = run(
            &repo,
            &[
                "source",
                "new",
                "sources/dima/other.md",
                "--project",
                "alpha",
                "--register-only",
                "--no-index",
                "--name",
                "notes",
                "--force",
            ],
            &[],
        );
        assert_eq!(code, 0, "--force replacement must succeed\nstdout:\n{stdout}\nstderr:\n{stderr}");

        // Exactly one registration named "notes", now pointing at the second file.
        let (stdout, stderr, code) = run(&repo, &["source", "list", "--project", "alpha"], &[]);
        assert_eq!(code, 0, "source list failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
        let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
        let sources = v["data"]["sources"].as_array().expect("sources array");
        let matches: Vec<&serde_json::Value> = sources.iter().filter(|s| s["name"].as_str() == Some("notes")).collect();
        assert_eq!(matches.len(), 1, "must be exactly one registration named 'notes', no duplicate: {stdout}");
        let path = matches[0]["path"].as_str().unwrap_or_default();
        assert!(path.contains("dima/other.md"), "registration must point at the replacement file: {stdout}");

        // mind-index.yaml projection is re-exported in sync, no stale entry.
        let index = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
        assert!(index.contains("dima/other.md"), "projection must reflect the replacement: {index}");
        assert!(!index.contains("file/notes.md"), "projection must not keep the replaced file's path: {index}");

        // `mf source status` agrees: no drift warning attached to the
        // replacement (the outer envelope's `warnings` array is omitted
        // entirely when empty, per its `skip_serializing_if`).
        let (stdout, stderr, code) = run(&repo, &["source", "status"], &[]);
        assert_eq!(code, 0, "source status failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
        let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
        if let Some(warnings) = v.get("warnings").and_then(|w| w.as_array()) {
            assert!(warnings.is_empty(), "replacement must not leave a drift warning: {stdout}");
        }
    }

    /// Spec 081 FR-002: the Lance backend preserves `added_at` across a
    /// `--force` replacement in storage, so the command's own JSON must report
    /// that preserved value — not a stamp minted while building the input
    /// model. The wait makes a fresh stamp distinguishable from the original at
    /// the timestamps' one-second resolution.
    #[test]
    fn force_replacement_json_reports_persisted_added_at_on_lance_backend() {
        let repo = provider_repo();
        let project = repo.path().join("projects/alpha");
        let before = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
        let original = added_at_for(&before, "notes").expect("notes must carry an added_at before the replacement");

        std::fs::create_dir_all(project.join("sources/dima")).unwrap();
        std::fs::write(project.join("sources/dima/other.md"), "second\n").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1_100));

        let (stdout, stderr, code) = run(
            &repo,
            &[
                "source",
                "new",
                "sources/dima/other.md",
                "--project",
                "alpha",
                "--register-only",
                "--no-index",
                "--name",
                "notes",
                "--force",
            ],
            &[],
        );
        assert_eq!(code, 0, "--force replacement must succeed\nstdout:\n{stdout}\nstderr:\n{stderr}");
        let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
        let reported = v["data"]["details"]["added_at"].as_str().expect("added_at in details");

        let after = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
        let persisted = added_at_for(&after, "notes").expect("notes must still carry an added_at");
        assert_eq!(persisted, original, "storage must keep the original creation time across --force: {after}");
        assert_eq!(reported, persisted, "command JSON must report the persisted added_at, not a fresh stamp: {stdout}");
    }

    /// Spec 081 FR-002: `--force` must say what it overwrote. Reporting only
    /// `replaced: true` leaves the caller unable to tell which registration was
    /// destroyed, and the text channel said nothing at all.
    #[test]
    fn force_replacement_discloses_the_overwritten_registration_on_lance_backend() {
        let repo = provider_repo();
        let project = repo.path().join("projects/alpha");
        std::fs::create_dir_all(project.join("sources/dima")).unwrap();
        std::fs::write(project.join("sources/dima/other.md"), "second\n").unwrap();

        let (stdout, stderr, code) = run(
            &repo,
            &[
                "source",
                "new",
                "sources/dima/other.md",
                "--project",
                "alpha",
                "--register-only",
                "--no-index",
                "--name",
                "notes",
                "--force",
            ],
            &[],
        );
        assert_eq!(code, 0, "--force replacement must succeed\nstdout:\n{stdout}\nstderr:\n{stderr}");
        assert!(
            stdout.contains("file/notes.md"),
            "JSON must identify the registration that was overwritten, not just replaced=true: {stdout}"
        );
    }

    /// Spec 081 FR-002: `source list` on the Lance backend dropped the
    /// timestamps the catalog had already read, reporting empty strings for
    /// every registration while the projection on disk held real values.
    #[test]
    fn source_list_reports_registration_timestamps_on_lance_backend() {
        let repo = provider_repo();
        let (stdout, stderr, code) = run(&repo, &["source", "list", "--project", "alpha"], &[]);
        assert_eq!(code, 0, "source list failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
        let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
        let sources = v["data"]["sources"].as_array().expect("sources array");
        let notes = sources.iter().find(|s| s["name"].as_str() == Some("notes")).expect("notes registration");
        assert!(
            !notes["added_at"].as_str().unwrap_or_default().is_empty(),
            "added_at must be reported, not blanked: {stdout}"
        );
        assert!(
            !notes["updated_at"].as_str().unwrap_or_default().is_empty(),
            "updated_at must be reported, not blanked: {stdout}"
        );
    }
}

// ---------------------------------------------------------------------------
// Spec 081 US1 follow-up: `--register-only --force` replaces a registration in
// place, so the record's identity-bearing fields must survive the swap. The
// legacy branch built a blank `Source` and overwrote the row wholesale, taking
// `added_at`, `tags` and the FR-011 passthrough map with it; the Lance branch
// preserves them but `source new` reported a freshly stamped `added_at` in its
// own output, and `source list` blanked both timestamps.
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
