use assert_cmd::Command;
use tempfile::TempDir;

mod common;

fn seed_sources(repo: &TempDir, project_name: &str) {
    let yaml = r#"schema_version: '1'
sources:
  - name: paper
    type: pdf
    path: sources/pdf/paper.pdf
    tags: []
    added_at: '2026-05-01T10:00:00Z'
    updated_at: '2026-05-01T10:00:00Z'
  - name: research-blog
    type: web
    url: https://example.com/research
    tags: []
    added_at: '2026-05-01T11:00:00Z'
    updated_at: '2026-05-01T11:00:00Z'
  - name: my-feed
    type: rss
    url: https://example.com/feed.xml
    tags: []
    added_at: '2026-05-01T12:00:00Z'
    updated_at: '2026-05-01T12:00:00Z'
"#;
    let yaml = yaml.replace("path: ~", "path:");
    common::write_index(repo, project_name, &yaml);
}

fn setup() -> (TempDir, std::path::PathBuf) {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    std::fs::create_dir_all(project.join("sources/pdf")).unwrap();
    std::fs::write(project.join("sources/pdf/paper.pdf"), b"fake pdf").unwrap();
    seed_sources(&repo, "alpha");
    (repo, project)
}

// ---------------------------------------------------------------------------
// 1. index_added_and_removed_summary — disk add + index remove → report correct
// ---------------------------------------------------------------------------

#[test]
fn index_added_and_removed_summary() {
    let (repo, project) = setup();
    // Add a new file on disk not in index
    std::fs::write(project.join("sources/pdf/new.pdf"), b"new pdf").unwrap();

    // Remove the indexed file from disk
    std::fs::remove_file(project.join("sources/pdf/paper.pdf")).unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "index", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("indexed source:"), "should use indexed format, got: {stdout}");
    assert!(stdout.contains("+1"), "should report 1 added, got: {stdout}");
    assert!(stdout.contains("-1"), "should report 1 removed, got: {stdout}");
}

// ---------------------------------------------------------------------------
// 2. index_keeps_url_sources_always — rss/web always kept
// ---------------------------------------------------------------------------

#[test]
fn index_keeps_url_sources_always() {
    let (repo, project) = setup();
    // Remove the indexed pdf from disk so it would be "removed"
    std::fs::remove_file(project.join("sources/pdf/paper.pdf")).unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "index", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    // Web and RSS sources should remain in the index
    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("research-blog"), "web source should be kept");
    assert!(index_content.contains("my-feed"), "rss source should be kept");
}

// ---------------------------------------------------------------------------
// 3. index_dry_run_no_writes — --dry-run doesn't modify index
// ---------------------------------------------------------------------------

#[test]
fn index_dry_run_no_writes() {
    let (repo, project) = setup();
    // Add a new file on disk
    std::fs::write(project.join("sources/pdf/new.pdf"), b"new pdf").unwrap();

    let before_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "index", "--project", "alpha", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("[dry-run]"), "dry-run output should have prefix, got: {stdout}");

    let after_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert_eq!(before_content, after_content, "index should not be modified");
}

// ---------------------------------------------------------------------------
// 4. index_missing_sources_dir — sources/ missing → usage hint
// ---------------------------------------------------------------------------

#[test]
fn index_missing_sources_dir() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    // Don't create sources/ dir

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "index", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("sources"), "stderr: {stderr}");
}

// ---------------------------------------------------------------------------
// 5. index_ignores_unknown_subdirs — sources/raw/ is ignored
// ---------------------------------------------------------------------------

#[test]
fn index_ignores_unknown_subdirs() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");

    // Create only sources/raw/ with a file (not sources/pdf/ or sources/file/)
    std::fs::create_dir_all(project.join("sources/raw")).unwrap();
    std::fs::write(project.join("sources/raw/data.bin"), b"data").unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "index", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    // Should have 0 added since raw/ is not scanned
    assert!(stdout.contains("=0"), "stdout: {stdout}");
}

// ---------------------------------------------------------------------------
// 6. index_ignores_hidden_files — .DS_Store / .gitkeep skipped
// ---------------------------------------------------------------------------

#[test]
fn index_ignores_hidden_files() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");

    std::fs::create_dir_all(project.join("sources/pdf")).unwrap();
    std::fs::write(project.join("sources/pdf/.gitkeep"), b"").unwrap();
    std::fs::write(project.join("sources/pdf/.DS_Store"), b"").unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "index", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.contains("gitkeep"), "should not index hidden files, got: {stdout}");
    assert!(stdout.contains("=0"), "stdout: {stdout}");
}

// ---------------------------------------------------------------------------
// 7. index_recursive_file_subdir — sources/file/sub/bar.md registered as file
// ---------------------------------------------------------------------------

#[test]
fn index_recursive_file_subdir() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");

    std::fs::create_dir_all(project.join("sources/file/sub")).unwrap();
    std::fs::write(project.join("sources/file/sub/bar.md"), b"# hello").unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "index", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("indexed source:"), "stdout: {stdout}");
    assert!(stdout.contains("+1"), "should report 1 added, got: {stdout}");

    // Index should have the entry with path including subdirectory
    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("sources/file/sub/bar.md"));
}

// ---------------------------------------------------------------------------
// 7b. index_recursive_source_kind_subdir — sources/yuque/sub/foo.md registered
// ---------------------------------------------------------------------------

#[test]
fn index_recursive_source_kind_subdir() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");

    std::fs::create_dir_all(project.join("sources/yuque/2025-05")).unwrap();
    std::fs::write(project.join("sources/yuque/2025-05/2025-05.md"), b"# monthly").unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "index", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("+1"), "should report 1 added, got: {stdout}");

    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("sources/yuque/2025-05/2025-05.md"));
    assert!(index_content.contains("source_kind: yuque"), "source_kind should be inferred, got: {index_content}");
}

// ---------------------------------------------------------------------------
// 8. index_does_not_modify_kept_metadata — kept entries unchanged
// ---------------------------------------------------------------------------

#[test]
fn index_does_not_modify_kept_metadata() {
    let (repo, project) = setup();

    let before_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    // Extract the added_at for paper
    assert!(before_content.contains("added_at: '2026-05-01T10:00:00Z'"));

    // Run index — paper.pdf exists on disk so it's kept
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "index", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let after_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    // The original kept entry should still have the same added_at
    // (quotes may differ after serde_yaml round-trip)
    assert!(
        after_content.contains("2026-05-01T10:00:00Z"),
        "kept entry added_at should be preserved, got: {after_content}"
    );
}

// ---------------------------------------------------------------------------
// 9. index_json_envelope — JSON mode has dry_run field
// ---------------------------------------------------------------------------

#[test]
fn index_json_envelope() {
    let (repo, project) = setup();
    // Add a new file to have some non-trivial report
    std::fs::write(project.join("sources/pdf/new.pdf"), b"new pdf").unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "index", "--project", "alpha", "--output", "json"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    assert!(v["data"]["dry_run"].is_boolean());
    assert!(v["data"]["kept_count"].is_number());
    assert!(v["data"]["added"].is_array());
    assert!(v["data"]["removed"].is_array());
}

// ---------------------------------------------------------------------------
// 10. index_idempotent — consistent state: no changes on re-run
// ---------------------------------------------------------------------------

#[test]
fn index_idempotent() {
    let (repo, project) = setup();

    // First run
    let output1 = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "index", "--project", "alpha"])
        .output()
        .unwrap();
    assert!(output1.status.success());

    let content1 = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();

    // Second run — should be identical
    let output2 = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "index", "--project", "alpha"])
        .output()
        .unwrap();
    assert!(output2.status.success());

    let content2 = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();

    // The second run should result in 0 added, 0 removed
    let stdout2 = String::from_utf8(output2.stdout).unwrap();
    assert!(!stdout2.contains("+ added"), "second run should have no added, got: {stdout2}");
    assert!(!stdout2.contains("- removed"), "second run should have no removed, got: {stdout2}");

    // The index content should be the same (modulo timing differences)
    assert_eq!(content1, content2, "index should be idempotent");
}

// ---------------------------------------------------------------------------
// 11. index_preserves_other_sections — articles/assets not wiped
// ---------------------------------------------------------------------------

#[test]
fn index_preserves_other_sections() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");

    // Create index with sources + articles + assets sections
    let yaml = r#"schema_version: '1'
sources:
  - name: paper
    type: pdf
    path: sources/pdf/paper.pdf
    tags: []
    added_at: '2026-05-01T10:00:00Z'
    updated_at: '2026-05-01T10:00:00Z'
articles:
  - title: my-post
    project: alpha
    type: blog
    article_path: docs/my-post.md
    status: draft
    created_at: '2026-05-01T10:00:00Z'
    updated_at: '2026-05-01T10:00:00Z'
assets:
  - name: logo
    type: image
    path: assets/logo.png
    size: 1024
    hash: abc
    tags: []
    added_at: '2026-05-01T10:00:00Z'
"#;
    common::write_index(&repo, "alpha", yaml);
    std::fs::create_dir_all(project.join("sources/pdf")).unwrap();
    std::fs::write(project.join("sources/pdf/paper.pdf"), b"fake pdf").unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "index", "--project", "alpha"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("articles:"), "articles section should be preserved");
    assert!(index_content.contains("my-post"), "article entry should be preserved");
    assert!(index_content.contains("assets:"), "assets section should be preserved");
    assert!(index_content.contains("logo"), "asset entry should be preserved");
}

// ── US3 (Bug #7): source index rebuild + idempotent ──

/// T019: idempotent re-run: +0 =N -0 when nothing changed.
#[test]
fn index_idempotent_rerun_no_changes() {
    let (repo, _project) = setup();
    // First run: index already matches disk
    let output1 = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "index", "--project", "alpha"])
        .output()
        .unwrap();
    assert!(output1.status.success(), "first run; stderr: {}", String::from_utf8_lossy(&output1.stderr));

    // Second run: nothing changed
    let output2 = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "index", "--project", "alpha"])
        .output()
        .unwrap();
    assert!(output2.status.success(), "second run; stderr: {}", String::from_utf8_lossy(&output2.stderr));
    let stdout2 = String::from_utf8(output2.stdout).unwrap();
    assert!(stdout2.contains("+0"), "should show 0 added: {stdout2}");
    assert!(stdout2.contains("=3"), "should show 3 kept: {stdout2}");
    assert!(stdout2.contains("-0"), "nothing should be removed: {stdout2}");
}

/// T019: rebuild from emptied index recovers files from disk.
#[test]
fn index_rebuilds_from_emptied_index() {
    let (repo, project) = setup();
    // Remove the index entries but leave the disk files
    let empty_index = "schema_version: '1'\n";
    common::write_index(&repo, "alpha", empty_index);

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "index", "--project", "alpha"])
        .output()
        .unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    // Should find the pdf file on disk
    assert!(stdout.contains("+1"), "should add 1 source from disk, got: {stdout}");

    // Verify the index was written with the recovered entry
    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("paper.pdf"), "recovered index should mention paper.pdf");
}

/// T019: files directly under sources/ (not in a named subdir) are discovered.
#[test]
fn index_discovers_top_level_sources_files() {
    let (repo, project) = setup();
    // Write a file directly under sources/
    std::fs::write(project.join("sources/notes.txt"), b"top level notes").unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "source", "index", "--project", "alpha"])
        .output()
        .unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("+1"), "should discover top-level sources/ file: {stdout}");

    // Check that the file is in the updated index
    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("notes"), "index must include top-level file");
}

// ---------------------------------------------------------------------------
// Spec 075 US2: Lance-mode `source index` is a disk-adoption/reconcile pass,
// not a `sources:`-list replay. T031-T040.
// ---------------------------------------------------------------------------

mod lance_index {
    use crate::common::embedding_provider::{provider_repo, run};

    /// T031/FR-015/FR-017: files present on disk but absent from the store are
    /// imported, and the pre-existing entry survives with its timestamps intact.
    #[test]
    fn index_imports_new_disk_files_and_keeps_existing_timestamps() {
        let repo = provider_repo();
        let project = repo.path().join("projects/alpha");
        std::fs::write(project.join("sources/file/second.md"), "A second note about tectonic plates.\n").unwrap();

        let before = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
        // The value alone, not the raw line: `mind-index.yaml` may hold
        // `sources:` as either a path-keyed mapping or a flat sequence, and
        // the two shapes indent an `added_at:` line differently even when
        // the timestamp itself is unchanged.
        let added_at_value = before
            .lines()
            .find(|l| l.trim_start().starts_with("added_at:"))
            .map(|l| l.trim_start().trim_start_matches("added_at:").trim().to_string())
            .expect("precondition: the pre-existing entry has an added_at");

        let (stdout, stderr, code) = run(&repo, &["source", "index", "--project", "alpha"], &[]);
        assert_eq!(code, 0, "index failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
        let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        let added = v["data"]["added"].as_array().unwrap();
        assert_eq!(added.len(), 1, "the new disk file must be imported\n{stdout}");
        assert_eq!(added[0]["path"], "sources/file/second.md");

        let after = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
        assert!(after.contains("second"), "new entry must be mirrored into the index\n{after}");
        assert!(after.contains(&added_at_value), "the pre-existing entry's added_at must survive untouched\n{after}");
    }

    /// T035/FR-016: a registration whose file is gone is reported, never
    /// removed by `source index` — only `mf source remove` may drop it.
    #[test]
    fn index_reports_missing_file_without_removing_the_registration() {
        let repo = provider_repo();
        let project = repo.path().join("projects/alpha");
        std::fs::remove_file(project.join("sources/file/notes.md")).unwrap();

        let (stdout, stderr, code) = run(&repo, &["source", "index", "--project", "alpha"], &[]);
        assert_eq!(code, 0, "index failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
        let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        let removed = v["data"]["removed"].as_array().unwrap();
        assert_eq!(removed.len(), 1, "the missing file must be reported\n{stdout}");
        assert_eq!(removed[0]["name"], "notes");

        let after = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
        assert!(after.contains("notes"), "a missing-file entry must not be deleted by index\n{after}");

        // Confirm it is still in the store, not just the YAML mirror.
        let (search_out, _, search_code) = run(&repo, &["source", "list", "--project", "alpha"], &[]);
        assert_eq!(search_code, 0, "{search_out}");
        assert!(search_out.contains("notes"), "the registration itself must survive\n{search_out}");
    }

    /// T034/FR-017: `--dry-run` reports exactly what the real run would do and
    /// writes nothing.
    #[test]
    fn index_dry_run_matches_real_run_and_writes_nothing() {
        let repo = provider_repo();
        let project = repo.path().join("projects/alpha");
        std::fs::write(project.join("sources/file/third.md"), "Notes on continental drift.\n").unwrap();

        let before = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
        let (dry_out, dry_err, dry_code) = run(&repo, &["source", "index", "--project", "alpha", "--dry-run"], &[]);
        assert_eq!(dry_code, 0, "dry-run failed\nstdout:\n{dry_out}\nstderr:\n{dry_err}");
        let after_dry = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
        assert_eq!(before, after_dry, "--dry-run must write nothing");

        let (real_out, real_err, real_code) = run(&repo, &["source", "index", "--project", "alpha"], &[]);
        assert_eq!(real_code, 0, "real run failed\nstdout:\n{real_out}\nstderr:\n{real_err}");

        let dv: serde_json::Value = serde_json::from_str(&dry_out).unwrap();
        let rv: serde_json::Value = serde_json::from_str(&real_out).unwrap();
        assert_eq!(dv["data"]["added"], rv["data"]["added"], "dry-run and real counts must agree");
    }

    /// T040/FR-019: a name collision during adoption raises the actionable
    /// naming error, not a generic file-conflict error.
    #[test]
    fn index_adoption_collision_is_actionable() {
        let repo = provider_repo();
        let project = repo.path().join("projects/alpha");
        // "notes" is already registered (from provider_repo); a second disk
        // file that would derive the same name must not silently clobber it.
        std::fs::create_dir_all(project.join("sources/file/nested")).unwrap();
        std::fs::write(project.join("sources/file/nested/notes.md"), "duplicate stem\n").unwrap();

        let (stdout, stderr, code) = run(&repo, &["source", "index", "--project", "alpha"], &[]);
        assert_ne!(code, 0, "a name collision during adoption must fail\nstdout:\n{stdout}\nstderr:\n{stderr}");
        assert!(
            stderr.contains("already registered") || stdout.contains("already registered"),
            "the actionable naming error must name the taken source\nstdout:\n{stdout}\nstderr:\n{stderr}"
        );
        assert!(
            !stderr.contains("overwrite existing file") && !stdout.contains("overwrite existing file"),
            "must not surface the generic file-conflict error\nstdout:\n{stdout}\nstderr:\n{stderr}"
        );
    }
}
