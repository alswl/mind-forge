//! E2E: Full workflow from quickstart.md — steps 1-8.
//!
//! Exercises US1, US2, US3, US4, US5, US6 across a single Mind Repo fixture.

use crate::helpers::*;

/// Run all 8 quickstart steps against a fresh temp repo.
#[test]
fn e2e_quickstart_workflow() {
    let ds = crate::datasets::Dataset::empty();
    let root = ds.root();

    // ── Step 1: Single-call authoring (US2 + US5 + US6) ──
    let (stdout, _stderr, code) = run_in(root, &["term", "new", "FooBar", "--alias", "foobar"]);
    assert_eq!(code, 0, "step 1: term new should succeed, got: {stdout} {_stderr}");
    assert!(stdout.contains("created term"), "step 1: created term: {stdout}");

    // ── Step 2: term new --alias is the canonical form (term add removed) ──
    let (stdout, stderr, code) = run_in(root, &["term", "new", "FooBar", "--alias", "foobar2"]);
    assert_eq!(code, 0, "step 2: term new --alias should succeed, got: {stdout} {stderr}");
    assert!(stderr.is_empty(), "step 2: canonical form should have clean stderr: {stderr}");

    // ── Step 3: Safe-by-default fix (US1) ──
    let sources_dir = root.join("sources");
    std::fs::create_dir_all(&sources_dir).unwrap();
    // "var60a" embeds 60 in an ASCII identifier → must NOT change.
    // "他叫 60。" has 60 standalone → must change.
    // CJK: 测试小文件 (小文 embedded between CJK chars, both neighbors CJK) → unchanged.
    // 小文 followed by ASCII space → standalone → changed.
    std::fs::write(sources_dir.join("notes.md"), "var60a is a symbol.\n他叫 60。\n测试小文件\n小文 负责备份\n")
        .unwrap();

    run_in(root, &["term", "new", "六十", "--misrecognition", "60"]);
    run_in(root, &["term", "new", "<name>", "--misrecognition", "小文"]);

    let (stdout, stderr, code) = run_in(root, &["term", "fix", "sources/notes.md", "-y", "--include-suggested"]);
    assert_eq!(code, 0, "step 3: fix should succeed, got: {stdout} {stderr}");

    let fixed = std::fs::read_to_string(sources_dir.join("notes.md")).unwrap();
    // var60a — identifier-internal is NOT changed by standalone boundary.
    assert!(fixed.contains("var60a"), "step 3: identifier-internal 60 should be unchanged: {fixed}");
    // 他叫 60。→ 他叫 六十。
    assert!(fixed.contains("他叫 六十"), "step 3: standalone 60 should be fixed: {fixed}");
    // 测试小文件 should be unchanged (CJK-embedded).
    assert!(fixed.contains("测试小文件"), "step 3: CJK-embedded 小文 should be unchanged: {fixed}");
    // 小文 standalone (followed by space) should be changed.
    assert!(fixed.contains("<name> 负责备份"), "step 3: standalone 小文 should be <name>: {fixed}");

    // ── Step 4: Discover global term from project (US3) ──
    run_in(root, &["term", "new", "Kubernetes", "--definition", "Container orchestrator"]);

    // Create a project for project-context tests.
    let project_dir = root.join("alpha");
    std::fs::create_dir_all(project_dir.join("docs")).unwrap();
    std::fs::write(project_dir.join("mind.yaml"), "schema_version: '1'\n").unwrap();
    std::fs::write(project_dir.join("mind-index.yaml"), "schema_version: '1'\n").unwrap();

    let (stdout, stderr, code) = run_in(root, &["-p", "alpha", "--json", "term", "show", "Kubernetes"]);
    assert_eq!(code, 0, "step 4: show should succeed, got: {stdout} {stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["data"]["scope"], "global", "step 4: scope should be global");

    // ── Step 5: Write against global-only term from project (US3, Q2=D0) ──
    let (stdout, stderr, code) = run_in(root, &["-p", "alpha", "term", "update", "Kubernetes", "--tag", "k8s"]);
    assert_eq!(code, 0, "step 5: update should succeed, got: {stdout} {stderr}");
    assert!(
        stderr.contains("WARN:") && stderr.contains("-p alpha was ignored"),
        "step 5: must emit silent-drop WARN: {stderr}"
    );

    // ── Step 6: Renamed --include-suggested flag ──
    // Create a fresh file with a misrecognition to lint against.
    std::fs::write(sources_dir.join("fresh.md"), "we use rag in production.\n").unwrap();
    run_in(root, &["term", "new", "RAG", "--misrecognition", "rag:RAG"]);
    run_in(root, &["term", "update", "RAG", "--correction-fix", "rag:suggested"]);

    let (_stdout, _stderr, code) =
        run_in(root, &["term", "fix", "sources/fresh.md", "--include-suggested", "-y", "--dry-run"]);
    assert_eq!(code, 1, "step 6: dry-run with --include-suggested should find suggested: {_stdout} {_stderr}");

    let (_stdout, stderr, code) = run_in(root, &["term", "lint", "sources/fresh.md", "--all"]);
    assert_eq!(code, 2, "step 6: removed --all must be rejected: {stderr}");
    assert!(stderr.contains("unexpected argument '--all'"), "step 6: stderr should identify --all: {stderr}");

    // ── Step 7: asset/source new rename (US5) ──
    let asset1 = root.join("icon.svg");
    let asset2 = root.join("logo.svg");
    std::fs::write(&asset1, b"<svg></svg>").unwrap();
    std::fs::write(&asset2, b"<svg></svg>").unwrap();

    let (_stdout, _stderr, code) = run_in(root, &["-p", "alpha", "asset", "new", &asset1.to_string_lossy()]);
    assert_eq!(code, 0, "step 7: asset new should succeed");

    let (_stdout, stderr, code) = run_in(root, &["-p", "alpha", "asset", "new", &asset2.to_string_lossy()]);
    assert_eq!(code, 0, "step 7: asset new should succeed");
    assert!(stderr.is_empty(), "step 7: asset new should have clean stderr: {stderr}");

    // ── Step 8: Path resolution sanity (US4) ──
    // All forms should address the same file.
    let (_stdout, _stderr, _code1) = run_in(root, &["-p", "alpha", "term", "lint", "sources/notes.md"]);
    // Missing file with resolved path in error.
    let (_stdout, stderr, code) = run_in(root, &["-p", "alpha", "term", "lint", "sources/missing.md"]);
    assert_ne!(code, 0, "step 8: missing file should error");
    assert!(stderr.contains("sources/missing.md"), "step 8: error must show literal input: {stderr}");
}
