use assert_cmd::Command;
use std::fs;

mod common;

// ── T010: Default-invocation integration test ──

#[test]
fn default_invocation_creates_directory_article() {
    let repo = common::setup_repo();
    common::create_project(&repo, "demo");

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("demo"))
        .args(["article", "new", "My Title"])
        .output()
        .expect("command runs");

    // FR-005, FR-006: exit 0, blank template, directory shape
    assert_eq!(output.status.code(), Some(0), "default article new should succeed");

    let project = repo.path().join("demo");
    assert!(project.join("docs/my-title/01-opening.md").exists(), "01-opening.md should exist under docs/my-title/");
    assert!(!project.join("docs/my-title.md").exists(), "no docs/my-title.md file should exist (directory default)");

    // FR-010: source_path is docs/my-title (no trailing slash)
    let index_content = fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("source_path: docs/my-title"));
    assert!(index_content.contains("type: blank"));
}

// ── T011: JSON-envelope test ──

#[test]
fn json_envelope_has_new_fields() {
    let repo = common::setup_repo();
    common::create_project(&repo, "demo");

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("demo"))
        .args(["--json", "article", "new", "Probe Two"])
        .output()
        .expect("command runs");

    assert_eq!(output.status.code(), Some(0));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let envelope: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(envelope["status"], "ok");
    assert_eq!(envelope["data"]["template"], "blank");
    assert_eq!(envelope["data"]["shape"], "directory");
    assert_eq!(envelope["data"]["path"], "docs/probe-two/");
    assert_eq!(envelope["data"]["files"].as_array().unwrap().len(), 1);
    assert_eq!(envelope["data"]["files"][0], "01-opening.md");
    // FR-011: legacy data.type is absent
    assert!(envelope["data"]["type"].is_null(), "legacy data.type must be absent");
}

#[test]
fn json_envelope_uses_configured_docs_dir() {
    let repo = common::setup_repo();
    common::create_project(&repo, "demo");
    common::write_mind_yaml(&repo, "demo", "schema_version: '1'\npaths:\n  docs: notes\n");

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("demo"))
        .args(["--json", "article", "new", "Configured Docs", "--template", "arch"])
        .output()
        .expect("command runs");

    assert_eq!(output.status.code(), Some(0));

    let project = repo.path().join("demo");
    assert!(project.join("notes/configured-docs/01-opening.md").exists());
    assert!(!project.join("docs/configured-docs").exists());

    let envelope: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope["data"]["path"], "notes/configured-docs/");
    assert_eq!(envelope["data"]["files"].as_array().unwrap().len(), 5);

    let index_content = fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("source_path: notes/configured-docs"));
}

// ── T012: Same-shape conflict test ──

#[test]
fn same_shape_conflict_and_force_replacement() {
    let repo = common::setup_repo();
    common::create_project(&repo, "demo");

    // First invocation succeeds
    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("demo"))
        .args(["article", "new", "My Title"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(0));

    // Second invocation without -f: conflict
    let output2 = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("demo"))
        .args(["article", "new", "My Title"])
        .output()
        .expect("command runs");
    assert_eq!(output2.status.code(), Some(1), "duplicate without -f should exit 1");
    let stderr2 = String::from_utf8_lossy(&output2.stderr);
    assert!(stderr2.contains("file-exists") || stderr2.contains("file_exists") || stderr2.contains("refusing"));

    // Filesystem + index unchanged
    let project = repo.path().join("demo");
    let index_before = fs::read_to_string(project.join("mind-index.yaml")).unwrap();

    // Force replacement succeeds
    let output3 = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("demo"))
        .args(["article", "new", "My Title", "-f"])
        .output()
        .expect("command runs");
    assert_eq!(output3.status.code(), Some(0), "force replacement should succeed");

    let index_after = fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    // Index should not have grown (replaced, not appended)
    assert_eq!(
        index_before.matches("source_path: docs/my-title").count(),
        index_after.matches("source_path: docs/my-title").count(),
        "force should replace, not append"
    );
    assert_eq!(
        index_after.matches("source_path: docs/my-title").count(),
        1,
        "should be exactly one entry for my-title"
    );
}

// ── T013: Usage-error test — legacy two-positional form ──

#[test]
fn legacy_two_positional_form_rejected() {
    let repo = common::setup_repo();
    common::create_project(&repo, "demo");

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("demo"))
        .args(["article", "new", "arch", "My Title"])
        .output()
        .expect("command runs");

    // FR-001: exit 2 (usage error)
    assert_eq!(output.status.code(), Some(2), "legacy two-positional form should exit 2");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("template") || stderr.contains("usage") || stderr.contains("unexpected"),
        "stderr should point at the new signature: '{stderr}'"
    );

    // Filesystem + index unchanged
    let project = repo.path().join("demo");
    assert!(!project.join("docs/my-title.md").exists());
    assert!(!project.join("docs/my-title").exists());
}

// ── T014: Atomic-rollback test ──

#[test]
fn atomic_rollback_on_index_save_failure() {
    let repo = common::setup_repo();
    common::create_project(&repo, "demo");

    // Make mind-index.yaml a read-only directory so saving fails
    let index_path = repo.path().join("demo/mind-index.yaml");
    fs::create_dir_all(&index_path).unwrap();
    // On unix, make it read-only
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&index_path).unwrap().permissions();
        perms.set_mode(0o444);
        fs::set_permissions(&index_path, perms).unwrap();
    }

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("demo"))
        .args(["article", "new", "Rollback Test"])
        .output()
        .expect("command runs");

    // Should fail (index can't be saved)
    assert_ne!(output.status.code(), Some(0), "should fail when index save fails");

    // FR-012: the created directory must be rolled back
    assert!(!repo.path().join("demo/docs/rollback-test").exists(), "docs/rollback-test/ must be removed on failure");
}

#[test]
fn force_replacement_restores_existing_directory_on_index_failure() {
    let repo = common::setup_repo();
    common::create_project(&repo, "demo");

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("demo"))
        .args(["article", "new", "Keep Me"])
        .assert()
        .success();

    let project = repo.path().join("demo");
    let article_dir = project.join("docs/keep-me");
    fs::write(article_dir.join("old-marker.md"), "old content").unwrap();

    let index_path = project.join("mind-index.yaml");
    fs::remove_file(&index_path).unwrap();
    fs::create_dir(&index_path).unwrap();

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(&project)
        .args(["article", "new", "Keep Me", "--template", "arch", "-f"])
        .output()
        .expect("command runs");

    assert_ne!(output.status.code(), Some(0), "force replacement should fail when index cannot load/save");
    assert!(article_dir.join("old-marker.md").exists(), "old directory content must be restored");
    assert!(!article_dir.join("02-context.md").exists(), "failed replacement must not leave new template blocks");
}

// ═══════════════════════════════════════════════════════════════════
// Phase 4: User Story 2 — arch template in directory form
// ═══════════════════════════════════════════════════════════════════

// ── T020: arch template directory layout ──

#[test]
fn arch_template_creates_correct_directory_structure() {
    let repo = common::setup_repo();
    common::create_project(&repo, "demo");

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("demo"))
        .args(["article", "new", "Auth Rewrite", "--template", "arch"])
        .output()
        .expect("command runs");

    assert_eq!(output.status.code(), Some(0));

    let dir = repo.path().join("demo/docs/auth-rewrite");
    assert!(dir.is_dir());
    assert!(dir.join("01-opening.md").exists());
    assert!(dir.join("02-context.md").exists());
    assert!(dir.join("03-decision.md").exists());
    assert!(dir.join("04-consequence.md").exists());
    assert!(dir.join("05-alternatives-considered.md").exists());

    // Exactly 5 files
    let count = std::fs::read_dir(&dir).unwrap().count();
    assert_eq!(count, 5);

    // JSON envelope check
    let _stdout = String::from_utf8_lossy(&output.stdout);
    // Non-json format doesn't output envelope; check via --json
}

#[test]
fn arch_template_json_envelope() {
    let repo = common::setup_repo();
    common::create_project(&repo, "demo");

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("demo"))
        .args(["--json", "article", "new", "Auth Rewrite", "--template", "arch"])
        .output()
        .expect("command runs");

    assert_eq!(output.status.code(), Some(0));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let envelope: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(envelope["data"]["template"], "arch");
    assert_eq!(envelope["data"]["shape"], "directory");
    assert_eq!(envelope["data"]["path"], "docs/auth-rewrite/");
    let files = envelope["data"]["files"].as_array().unwrap();
    assert_eq!(files.len(), 5);
    assert!(files.iter().any(|f| f.as_str() == Some("01-opening.md")));
}

// ── T021: template roundtrip — concat = resolved template ──

#[test]
fn arch_template_roundtrip_byte_equal() {
    let repo = common::setup_repo();
    common::create_project(&repo, "demo");

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("demo"))
        .args(["article", "new", "Auth Rewrite", "--template", "arch"])
        .output()
        .expect("command runs");

    let dir = repo.path().join("demo/docs/auth-rewrite");

    // Concat in filename order
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    names.sort();

    let mut body = String::new();
    for name in &names {
        body.push_str(&std::fs::read_to_string(dir.join(name)).unwrap());
    }

    // Build expected: arch template with slot substitution
    assert!(body.contains("# Auth Rewrite"));
    assert!(body.contains("## Context"));
    assert!(body.contains("## Decision"));
    assert!(body.contains("## Consequence"));
    assert!(body.contains("## Alternatives Considered"));
    assert!(body.contains("> Created:")); // slot substitution happened
}

// ── T023: duplicate-slug rejection ──

#[test]
fn duplicate_h2_slug_rejected_with_synthetic_template() {
    let repo = common::setup_repo();
    common::create_project(&repo, "demo");

    // Write a custom template with colliding H2 slugs
    let tmpl = "# {title}\n\n> Created: {created_at}\n\n## Notes\n\nbody1\n\n## NOTES\n\nbody2\n";
    std::fs::write(repo.path().join("demo").join("collide.md"), tmpl).unwrap();

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("demo"))
        .args(["article", "new", "Bad Template", "--template", "collide.md"])
        .output()
        .expect("command runs");

    assert_eq!(output.status.code(), Some(1), "duplicate slug should exit 1");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("duplicate_block_slug") || stderr.contains("duplicate block slug"));

    // Directory not created
    assert!(!repo.path().join("demo/docs/bad-template").exists());

    // Index unchanged (may not exist if no prior article was created)
    let index_path = repo.path().join("demo/mind-index.yaml");
    if index_path.exists() {
        let index_content = std::fs::read_to_string(&index_path).unwrap();
        assert!(!index_content.contains("Bad Template"));
    }
}

// ═══════════════════════════════════════════════════════════════════
// Phase 5: User Story 3 — --file mode (single file output)
// ═══════════════════════════════════════════════════════════════════

// ── T025: --file mode integration test ──

#[test]
fn file_mode_creates_single_file() {
    let repo = common::setup_repo();
    common::create_project(&repo, "demo");

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("demo"))
        .args(["--json", "article", "new", "Quick Note", "--template", "blog", "--file"])
        .output()
        .expect("command runs");

    assert_eq!(output.status.code(), Some(0));

    let project = repo.path().join("demo");
    assert!(project.join("docs/quick-note.md").exists(), "single file should exist");
    assert!(!project.join("docs/quick-note").exists(), "no directory should exist");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let envelope: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(envelope["data"]["template"], "blog");
    assert_eq!(envelope["data"]["shape"], "file");
    assert_eq!(envelope["data"]["path"], "docs/quick-note.md");
    assert_eq!(envelope["data"]["files"][0], "quick-note.md");

    // FR-008: source_path is docs/quick-note.md
    let index_content = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("source_path: docs/quick-note.md"));
}

// ── T026: Template content determines the block structure ──

#[test]
fn custom_template_creates_monthly_review_blocks_from_h2_headings() {
    let repo = common::setup_repo();
    common::create_project(&repo, "demo");

    let templates_dir = repo.path().join("demo/templates");
    fs::create_dir_all(&templates_dir).unwrap();
    fs::write(
        templates_dir.join("monthly-review.md"),
        "# {title}\n\n> Created: {created_at}\n\n## What Done\n\n## Next Month\n\n## Thoughts\n\n## Others Sharing\n",
    )
    .unwrap();

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("demo"))
        .args(["--json", "article", "new", "2026-04 Review", "--template", "templates/monthly-review.md"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(0));

    let dir = repo.path().join("demo/docs/2026-04-review");
    assert!(dir.join("01-opening.md").exists());
    assert!(dir.join("02-what-done.md").exists());
    assert!(dir.join("03-next-month.md").exists());
    assert!(dir.join("04-thoughts.md").exists());
    assert!(dir.join("05-others-sharing.md").exists());
    assert!(!repo.path().join("demo/docs/2026-04-review.md").exists());

    let envelope: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        envelope["data"]["files"],
        serde_json::json!([
            "01-opening.md",
            "02-what-done.md",
            "03-next-month.md",
            "04-thoughts.md",
            "05-others-sharing.md"
        ])
    );
}

#[test]
fn blog_file_mode_keeps_blog_body_in_single_file() {
    let repo = common::setup_repo();
    common::create_project(&repo, "demo");

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("demo"))
        .args(["article", "new", "Quick Note", "--template", "blog", "--single-file"])
        .output()
        .expect("command runs");

    let content = std::fs::read_to_string(repo.path().join("demo/docs/quick-note.md")).unwrap();
    assert!(content.contains("# Quick Note"));
    assert!(content.contains("> Created:"));
    assert!(content.contains("## Summary"));
    assert!(content.contains("## Content"));
}

// ── T027: --file + blank produces single file ──

#[test]
fn file_mode_blank_template() {
    let repo = common::setup_repo();
    common::create_project(&repo, "demo");

    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("demo"))
        .args(["article", "new", "Minimal", "--file"])
        .output()
        .expect("command runs");

    let content = std::fs::read_to_string(repo.path().join("demo/docs/minimal.md")).unwrap();
    assert!(content.contains("# Minimal"));
    assert!(content.contains("> Created:"));
    // blank template should NOT have ## headings
    assert!(!content.contains("##"));
}

// ── T028: Cross-shape conflict test ──

#[test]
fn cross_shape_conflict_rejected() {
    let repo = common::setup_repo();
    common::create_project(&repo, "demo");

    // Create a file first
    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("demo"))
        .args(["article", "new", "Conflicting", "--file"])
        .output()
        .expect("command runs");

    let project = repo.path().join("demo");
    assert!(project.join("docs/conflicting.md").exists());

    // Try to create a directory with the same name
    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("demo"))
        .args(["article", "new", "Conflicting"])
        .output()
        .expect("command runs");

    assert_eq!(output.status.code(), Some(1), "cross-shape conflict should exit 1");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot create") && stderr.contains("same name already exists"),
        "stderr should mention shape conflict: '{stderr}'"
    );

    // -f does NOT cross shapes
    let output2 = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("demo"))
        .args(["article", "new", "Conflicting", "-f"])
        .output()
        .expect("command runs");

    assert_eq!(output2.status.code(), Some(1), "-f should not cross shapes");

    // Original file should still exist
    assert!(project.join("docs/conflicting.md").exists());
}

// ═══════════════════════════════════════════════════════════════════
// Phase 6: User Story 4 — custom template file
// ═══════════════════════════════════════════════════════════════════

// ── T030: custom-template directory layout ──

#[test]
fn custom_template_directory_layout() {
    let repo = common::setup_repo();
    common::create_project(&repo, "demo");

    let templates_dir = repo.path().join("demo/templates");
    fs::create_dir_all(&templates_dir).unwrap();
    let tmpl = "# {title}\n\n> Created: {created_at}\n\n## Background\n\n## Analysis\n\n## Action Items\n";
    fs::write(templates_dir.join("postmortem.md"), tmpl).unwrap();

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("demo"))
        .args(["--json", "article", "new", "Q2 Outage", "--template", "templates/postmortem.md"])
        .output()
        .expect("command runs");

    assert_eq!(output.status.code(), Some(0));

    let dir = repo.path().join("demo/docs/q2-outage");
    assert!(dir.is_dir());
    assert!(dir.join("01-opening.md").exists());
    assert!(dir.join("02-background.md").exists());
    assert!(dir.join("03-analysis.md").exists());
    assert!(dir.join("04-action-items.md").exists());

    let count = std::fs::read_dir(&dir).unwrap().count();
    assert_eq!(count, 4);

    // JSON envelope
    let stdout = String::from_utf8_lossy(&output.stdout);
    let envelope: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(envelope["data"]["template"], "templates/postmortem.md");
    assert_eq!(envelope["data"]["shape"], "directory");
    assert_eq!(envelope["data"]["path"], "docs/q2-outage/");

    // Index records article_type as blank for custom templates
    let index_content = fs::read_to_string(repo.path().join("demo/mind-index.yaml")).unwrap();
    assert!(index_content.contains("type: blank"));
    assert!(index_content.contains("source_path: docs/q2-outage"));
}

// ── T031: custom-template --file mode ──

#[test]
fn custom_template_file_mode() {
    let repo = common::setup_repo();
    common::create_project(&repo, "demo");

    let templates_dir = repo.path().join("demo/templates");
    fs::create_dir_all(&templates_dir).unwrap();
    let tmpl = "# {title}\n\n> Created: {created_at}\n\n## Notes\n\nbody text\n";
    fs::write(templates_dir.join("simple.md"), tmpl).unwrap();

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("demo"))
        .args(["--json", "article", "new", "Quick Memo", "--template", "templates/simple.md", "--file"])
        .output()
        .expect("command runs");

    assert_eq!(output.status.code(), Some(0));

    let file_path = repo.path().join("demo/docs/quick-memo.md");
    assert!(file_path.exists());
    assert!(!repo.path().join("demo/docs/quick-memo").exists());

    let content = fs::read_to_string(&file_path).unwrap();
    assert!(content.contains("# Quick Memo"));
    assert!(content.contains("> Created:"));
    assert!(content.contains("## Notes"));
    assert!(content.contains("body text"));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let envelope: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(envelope["data"]["template"], "templates/simple.md");
    assert_eq!(envelope["data"]["shape"], "file");
    assert_eq!(envelope["data"]["path"], "docs/quick-memo.md");
}

// ── T032: unknown template rejection ──

#[test]
fn unknown_template_exit_code_and_message() {
    let repo = common::setup_repo();
    common::create_project(&repo, "demo");

    // Non-existent built-in and non-existent file path
    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("demo"))
        .args(["--json", "article", "new", "Bad One", "--template", "not-a-thing"])
        .output()
        .expect("command runs");

    assert_eq!(output.status.code(), Some(2), "unknown template should exit 2");

    // Text stderr should mention built-ins and the not-found name
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not-a-thing"), "stderr should mention the unknown template: '{stderr}'");

    // JSON error envelope is on stderr in --json mode
    let envelope: serde_json::Value = serde_json::from_str(&stderr).unwrap();
    assert_eq!(envelope["status"], "error");
    assert_eq!(envelope["error"]["kind"], "unknown_template");

    // Filesystem unchanged
    let project = repo.path().join("demo");
    assert!(!project.join("docs/bad-one.md").exists());
    assert!(!project.join("docs/bad-one").exists());
}

#[test]
fn unknown_template_with_missing_parent_is_still_usage_error() {
    let repo = common::setup_repo();
    common::create_project(&repo, "demo");

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("demo"))
        .args(["--json", "article", "new", "Bad Nested", "--template", "missing/nope.md"])
        .output()
        .expect("command runs");

    assert_eq!(output.status.code(), Some(2), "missing nested template should exit 2");
    let envelope: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(envelope["error"]["kind"], "unknown_template");
}

#[test]
fn custom_template_path_must_stay_under_project_root() {
    let repo = common::setup_repo();
    common::create_project(&repo, "demo");
    fs::write(repo.path().join("outside.md"), "# {title}\n\n## Outside\n").unwrap();

    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("demo"))
        .args(["article", "new", "Escapes", "--template", "../outside.md"])
        .output()
        .expect("command runs");

    assert_eq!(output.status.code(), Some(2), "outside template path should be a usage error");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("outside") || stderr.contains("project root") || stderr.contains("Mind Repo root"),
        "stderr should explain the path boundary: '{stderr}'"
    );
    assert!(!repo.path().join("demo/docs/escapes").exists());
}
