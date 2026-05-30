//! Real boundary tests for 039 contracts.
//!
//! Targets specific behaviors that the existing e2e suite skirts past with
//! permissive inputs (`--max-warnings 999`, `--severity info`, no CJK fixtures,
//! no manifest re-read after destructive ops). Each test drives the CLI with
//! a fixture engineered to land *on* the boundary that the spec promises.

use std::fs;
use std::path::Path;

use serde_json::Value;
use serde_yaml::Value as YamlValue;

use crate::helpers::*;

fn write_file(path: impl AsRef<Path>, contents: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dir");
    }
    fs::write(path, contents).expect("write fixture file");
}

fn run_rooted(repo: &TempDir, args: &[&str]) -> (String, String, i32) {
    let root = repo.path().to_string_lossy().into_owned();
    let mut full = vec!["--root", root.as_str()];
    full.extend_from_slice(args);
    run_in(repo.path(), &full)
}

fn decode_data(stdout: &str) -> serde_json::Map<String, Value> {
    let value: Value = serde_json::from_str(stdout).expect("stdout should be JSON");
    assert_eq!(value["status"], "ok", "envelope not ok: {stdout}");
    value["data"].as_object().expect("data should be object").clone()
}

// ── Fixture: project with 1 Error + 2 Warnings ─────────────────────────────
//
// MissingDirectory   (Error)   ← sources/ deleted
// StaleIndexEntry    (Warning) ← index references docs/ghost.md (absent)
// NameConvention     (Warning) ← index references docs/BadName.md (present, camelCase)
fn project_lint_mixed_severity_fixture() -> TempDir {
    let repo = TempDir::new().expect("temp repo");
    let root = repo.path();
    write_file(
        root.join("minds.yaml"),
        "schema_version: '1'\nprojects_dir: '.'\nprojects:\n  - name: alpha\n    path: ./alpha\n    created_at: '2026-05-30T00:00:00Z'\n    archived_at: ~\n",
    );
    write_file(root.join("alpha/mind.yaml"), "schema_version: '1'\n");
    fs::create_dir_all(root.join("alpha/docs")).unwrap();
    fs::create_dir_all(root.join("alpha/assets")).unwrap();
    // sources/ intentionally missing
    write_file(root.join("alpha/docs/BadName.md"), "# Bad Name\n\nbody\n");
    write_file(
        root.join("alpha/mind-index.yaml"),
        r#"schema_version: '1'
articles:
  - title: "Ghost"
    project: "alpha"
    article_type: blog
    article_path: "docs/ghost.md"
    status: draft
    created_at: "2026-05-30T00:00:00Z"
    updated_at: "2026-05-30T00:00:00Z"
  - title: "Bad Name"
    project: "alpha"
    article_type: blog
    article_path: "docs/BadName.md"
    status: draft
    created_at: "2026-05-30T00:00:00Z"
    updated_at: "2026-05-30T00:00:00Z"
"#,
    );
    repo
}

// ── Fixture: project with 0 Errors + 2 Warnings (max-warnings boundary) ────
fn project_lint_warnings_only_fixture() -> TempDir {
    let repo = TempDir::new().expect("temp repo");
    let root = repo.path();
    write_file(
        root.join("minds.yaml"),
        "schema_version: '1'\nprojects_dir: '.'\nprojects:\n  - name: alpha\n    path: ./alpha\n    created_at: '2026-05-30T00:00:00Z'\n    archived_at: ~\n",
    );
    write_file(root.join("alpha/mind.yaml"), "schema_version: '1'\n");
    fs::create_dir_all(root.join("alpha/docs")).unwrap();
    fs::create_dir_all(root.join("alpha/assets")).unwrap();
    fs::create_dir_all(root.join("alpha/sources")).unwrap();
    write_file(root.join("alpha/docs/BadName.md"), "# Bad\n\nbody\n");
    write_file(
        root.join("alpha/mind-index.yaml"),
        r#"schema_version: '1'
articles:
  - title: "Ghost"
    project: "alpha"
    article_type: blog
    article_path: "docs/ghost.md"
    status: draft
    created_at: "2026-05-30T00:00:00Z"
    updated_at: "2026-05-30T00:00:00Z"
  - title: "Bad Name"
    project: "alpha"
    article_type: blog
    article_path: "docs/BadName.md"
    status: draft
    created_at: "2026-05-30T00:00:00Z"
    updated_at: "2026-05-30T00:00:00Z"
"#,
    );
    repo
}

// ── Fixture: article lint with 1 Error + 1 Warning ─────────────────────────
//   empty_file          (Error)   ← docs/empty.md is empty
//   filename_convention (Warning) ← docs/BadName.md has uppercase
fn article_lint_mixed_severity_fixture() -> TempDir {
    let repo = TempDir::new().expect("temp repo");
    let root = repo.path();
    write_file(
        root.join("minds.yaml"),
        "schema_version: '1'\nprojects_dir: '.'\nprojects:\n  - name: alpha\n    path: ./alpha\n    created_at: '2026-05-30T00:00:00Z'\n    archived_at: ~\n",
    );
    write_file(root.join("alpha/mind.yaml"), "schema_version: '1'\n");
    fs::create_dir_all(root.join("alpha/docs")).unwrap();
    fs::create_dir_all(root.join("alpha/assets")).unwrap();
    fs::create_dir_all(root.join("alpha/sources")).unwrap();
    write_file(root.join("alpha/docs/empty.md"), "");
    write_file(root.join("alpha/docs/BadName.md"), "# Bad\n\nbody\n");
    write_file(root.join("alpha/mind-index.yaml"), "schema_version: '1'\n");
    repo
}

// ── Fixture: term lint with at least one finding ──────────────────────────
fn term_lint_with_finding_fixture() -> TempDir {
    let repo = TempDir::new().expect("temp repo");
    let root = repo.path();
    write_file(
        root.join("minds.yaml"),
        "schema_version: '1'\nprojects_dir: '.'\nprojects:\n  - name: alpha\n    path: ./alpha\n    created_at: '2026-05-30T00:00:00Z'\n    archived_at: ~\n",
    );
    write_file(root.join("alpha/mind.yaml"), "schema_version: '1'\n");
    fs::create_dir_all(root.join("alpha/docs")).unwrap();
    fs::create_dir_all(root.join("alpha/assets")).unwrap();
    fs::create_dir_all(root.join("alpha/sources")).unwrap();
    // Term "RAG" with a correction: rag → RAG (Correction schema: {original, correct})
    write_file(
        root.join("alpha/mind-index.yaml"),
        r#"schema_version: '1'
terms:
  - term: RAG
    definition: Retrieval-Augmented Generation
    aliases: []
    tags: []
    corrections:
      - original: rag
        correct: RAG
"#,
    );
    // Article that contains the incorrect form → term lint must find it
    write_file(root.join("alpha/docs/post.md"), "# Post\n\nWe use rag everywhere.\n");
    repo
}

// ── Fixture: CJK asset path ────────────────────────────────────────────────
fn cjk_asset_fixture() -> TempDir {
    let repo = TempDir::new().expect("temp repo");
    let root = repo.path();
    write_file(
        root.join("minds.yaml"),
        "schema_version: '1'\nprojects_dir: '.'\nprojects:\n  - name: alpha\n    path: ./alpha\n    created_at: '2026-05-30T00:00:00Z'\n    archived_at: ~\n",
    );
    write_file(root.join("alpha/mind.yaml"), "schema_version: '1'\n");
    fs::create_dir_all(root.join("alpha/docs")).unwrap();
    fs::create_dir_all(root.join("alpha/sources")).unwrap();
    let cjk_path = "assets/资料/封面图.png";
    write_file(root.join("alpha").join(cjk_path), "png-bytes\n");
    // Also include a plain-ASCII asset to verify both render side by side
    write_file(root.join("alpha/assets/logo.png"), "png\n");
    write_file(
        root.join("alpha/mind-index.yaml"),
        r#"schema_version: '1'
assets:
  - name: "logo.png"
    type: image
    path: "assets/logo.png"
    size: 4
    hash: "aaaa"
    tags: []
    added_at: "2026-05-30T00:00:00Z"
  - name: "封面图.png"
    type: image
    path: "assets/资料/封面图.png"
    size: 10
    hash: "bbbb"
    tags: []
    added_at: "2026-05-30T00:00:00Z"
"#,
    );
    repo
}

// ──────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────

#[test]
fn e2e_boundary_project_lint_severity_filter_recomputes_summary() {
    let repo = project_lint_mixed_severity_fixture();

    // Observed baseline: 1 Error (missing_directory) + 3 Warnings
    //   ▸ stale_index_entry on docs/ghost.md
    //   ▸ name_convention on docs/ghost.md       ← see Finding #1 below
    //   ▸ name_convention on docs/BadName.md
    let (stdout, _, _) = run_rooted(
        &repo,
        &["--project", "alpha", "--json", "project", "lint", "--severity", "info", "--max-warnings", "999"],
    );
    let data = decode_data(&stdout);
    let summary = &data["summary"];
    assert_eq!(summary["errors"], 1, "baseline errors: {summary}");
    assert!(
        summary["warnings"].as_u64().unwrap() >= 2,
        "baseline warnings must be ≥2 to exercise the filter: {summary}"
    );
    let baseline_warnings = summary["warnings"].as_u64().unwrap();

    // Apply --severity error: only the 1 Error should be visible AND counted.
    let (stdout, _, _) = run_rooted(
        &repo,
        &["--project", "alpha", "--json", "project", "lint", "--severity", "error", "--max-warnings", "999"],
    );
    let data = decode_data(&stdout);
    let issues = data["issues"].as_array().expect("issues array");
    assert_eq!(issues.len(), 1, "severity=error should keep 1 issue: {stdout}");

    let summary = &data["summary"];
    assert_eq!(summary["errors"], 1, "summary.errors should be 1 after severity filter: {summary}");
    // Contract (T088 / FR-US7): warnings filtered out → summary.warnings == 0.
    // Bug surface: current handler returns the *unfiltered* summary, so this
    // is expected to fail until src/cli/project.rs recomputes the summary
    // from the filtered `issues` slice.
    assert_eq!(
        summary["warnings"], 0,
        "summary.warnings must reflect post-filter count (baseline {baseline_warnings}): {summary}\nfull stdout: {stdout}"
    );
}

#[test]
fn e2e_boundary_project_lint_max_warnings_threshold() {
    let repo = project_lint_warnings_only_fixture();

    // Discover actual warning count from the fixture (depends on how many .md
    // entries fire name_convention). Test the boundary regardless of the exact
    // count, by computing the four critical exit codes off the observed N.
    let (stdout, _, _) = run_rooted(
        &repo,
        &["--project", "alpha", "--json", "project", "lint", "--severity", "info", "--max-warnings", "999"],
    );
    let data = decode_data(&stdout);
    assert_eq!(data["summary"]["errors"], 0, "fixture has no errors: {stdout}");
    let n = data["summary"]["warnings"].as_u64().expect("warnings count") as i32;
    assert!(n >= 2, "fixture must produce ≥2 warnings to exercise boundary, got {n}");

    // Boundary cases (0 errors, N warnings):
    //   --max-warnings N-1 → exit 1 (N > N-1)
    //   --max-warnings N   → exit 0 (N == N) ← critical equality
    //   --max-warnings N+1 → exit 0 (N < N+1)
    //   --max-warnings 0   → exit 1 (N > 0)
    let cases = [(0, 1), (n - 1, 1), (n, 0), (n + 1, 0)];
    for (max, expected_exit) in cases {
        let max_str = max.to_string();
        let args = ["--project", "alpha", "--json", "project", "lint", "--max-warnings", &max_str];
        let (stdout, stderr, code) = run_rooted(&repo, &args);
        assert_eq!(
            code, expected_exit,
            "--max-warnings {max} (N={n}) expected exit {expected_exit}, got {code}\nstdout: {stdout}\nstderr: {stderr}"
        );
    }
}

#[test]
fn e2e_boundary_article_lint_severity_filter_recomputes_summary() {
    let repo = article_lint_mixed_severity_fixture();

    // Baseline: see both issues
    let (stdout, _, _) = run_rooted(
        &repo,
        &["--project", "alpha", "--json", "article", "lint", "--severity", "info", "--max-warnings", "999"],
    );
    let data = decode_data(&stdout);
    assert_eq!(data["issues"].as_array().unwrap().len(), 2, "baseline 2 issues: {stdout}");
    let summary = &data["summary"];
    assert_eq!(summary["errors"], 1, "baseline errors: {summary}");
    assert_eq!(summary["warnings"], 1, "baseline warnings: {summary}");

    // --severity error: keep only the Error AND zero the warnings counter
    let (stdout, _, _) = run_rooted(
        &repo,
        &["--project", "alpha", "--json", "article", "lint", "--severity", "error", "--max-warnings", "999"],
    );
    let data = decode_data(&stdout);
    assert_eq!(data["issues"].as_array().unwrap().len(), 1, "filtered to 1: {stdout}");
    let summary = &data["summary"];
    assert_eq!(summary["errors"], 1, "summary.errors=1: {summary}");
    assert_eq!(summary["warnings"], 0, "summary.warnings=0 after filter: {summary}");
}

// Deferred: term lint's `--rule` / `--severity` flags are declared via the
// shared `LintFlags` struct but TermFinding has no rule/severity dimension,
// so spec T091/T092 collides with the data model. Holding both tests until
// term lint is redesigned as its own slice.
#[test]
#[ignore = "term lint rule/severity semantics deferred — see spec gap notes"]
fn e2e_boundary_term_lint_rule_filter_actually_filters() {
    let repo = term_lint_with_finding_fixture();

    // Baseline: at least one finding present (the "rag → RAG" correction in
    // post.md). Read-only term lint exits 1 when findings exist — that's per
    // spec, not a failure for this test. We only care about counting.
    let (stdout, _, _) = run_rooted(
        &repo,
        &["--project", "alpha", "--json", "term", "lint", "--severity", "info", "--max-warnings", "999"],
    );
    let data = decode_data(&stdout);
    let findings = data.get("findings").and_then(|v| v.as_array()).expect("findings array");
    assert!(!findings.is_empty(), "fixture should produce ≥1 finding, got: {stdout}");
    let baseline_count = findings.len();

    // --rule with an unknown rule name should drop all findings (or error out).
    // Currently the handler silently ignores --rule, so findings.len() stays the same.
    let (stdout, _, _) = run_rooted(
        &repo,
        &["--project", "alpha", "--json", "term", "lint", "--rule", "definitely-not-a-rule", "--max-warnings", "999"],
    );
    let data = decode_data(&stdout);
    let findings = data.get("findings").and_then(|v| v.as_array()).expect("findings array");
    assert!(
        findings.len() < baseline_count,
        "--rule with unknown rule must filter findings (baseline {baseline_count}, got {}): {stdout}",
        findings.len()
    );
}

#[test]
#[ignore = "term lint rule/severity semantics deferred — see spec gap notes"]
fn e2e_boundary_term_lint_severity_filter_actually_filters() {
    let repo = term_lint_with_finding_fixture();

    let (stdout, _, _) = run_rooted(
        &repo,
        &["--project", "alpha", "--json", "term", "lint", "--severity", "info", "--max-warnings", "999"],
    );
    let baseline = decode_data(&stdout);
    let baseline_count = baseline.get("findings").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
    assert!(baseline_count > 0, "baseline must produce findings");

    // --severity error: term findings have no severity field. Spec T092 ("apply
    // the filter to the issue list") implies the filter must mean *something*
    // — either treat all findings as a default severity and drop those below
    // threshold, or refuse the flag. Silently returning the full list is the
    // bug.
    let (stdout, _, _) = run_rooted(
        &repo,
        &["--project", "alpha", "--json", "term", "lint", "--severity", "error", "--max-warnings", "999"],
    );
    let data = decode_data(&stdout);
    let filtered_count = data.get("findings").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
    assert!(
        filtered_count < baseline_count,
        "--severity error must change finding count (baseline {baseline_count}, got {filtered_count}): {stdout}"
    );
}

#[test]
fn e2e_boundary_lint_name_convention_accepts_kebab_case_md() {
    // A textbook-kebab-case article (`first-post.md`) must NOT trigger
    // name_convention. Currently util::to_filename treats `.` as a separator,
    // so "first-post.md" → "first-post-md" ≠ "first-post.md" and the lint
    // mis-flags every `.md` file. This is a real product bug surfaced by the
    // mixed-severity fixture; this test is its minimal repro.
    let repo = TempDir::new().expect("temp repo");
    let root = repo.path();
    write_file(
        root.join("minds.yaml"),
        "schema_version: '1'\nprojects_dir: '.'\nprojects:\n  - name: alpha\n    path: ./alpha\n    created_at: '2026-05-30T00:00:00Z'\n    archived_at: ~\n",
    );
    write_file(root.join("alpha/mind.yaml"), "schema_version: '1'\n");
    fs::create_dir_all(root.join("alpha/docs")).unwrap();
    fs::create_dir_all(root.join("alpha/assets")).unwrap();
    fs::create_dir_all(root.join("alpha/sources")).unwrap();
    write_file(root.join("alpha/docs/first-post.md"), "# First Post\n\nbody\n");
    write_file(
        root.join("alpha/mind-index.yaml"),
        r#"schema_version: '1'
articles:
  - title: First Post
    project: alpha
    article_type: blog
    article_path: docs/first-post.md
    status: draft
    created_at: '2026-05-30T00:00:00Z'
    updated_at: '2026-05-30T00:00:00Z'
"#,
    );

    let (stdout, _, _) = run_rooted(
        &repo,
        &["--project", "alpha", "--json", "project", "lint", "--rule", "name_convention", "--severity", "info"],
    );
    let data = decode_data(&stdout);
    let issues = data["issues"].as_array().expect("issues");
    assert!(issues.is_empty(), "kebab-case `first-post.md` should not violate name_convention, got: {stdout}");
}

#[test]
fn e2e_boundary_remove_project_purges_manifest_entry() {
    let repo = TempDir::new().expect("temp repo");
    let root = repo.path();
    write_file(
        root.join("minds.yaml"),
        r#"schema_version: '1'
projects_dir: '.'
projects:
  - name: alpha
    path: ./alpha
    created_at: '2026-05-30T00:00:00Z'
    archived_at: ~
  - name: beta
    path: ./beta
    created_at: '2026-05-30T00:00:00Z'
    archived_at: ~
"#,
    );
    write_file(root.join("alpha/mind.yaml"), "schema_version: '1'\n");
    write_file(root.join("beta/mind.yaml"), "schema_version: '1'\n");

    // Extract project identifiers from manifest. The on-disk schema allows
    // either `- name: foo` objects or bare path strings (Python `mind` 0.3.0
    // compat); we accept both shapes.
    fn project_ids(manifest: &YamlValue) -> Vec<String> {
        manifest["projects"]
            .as_sequence()
            .map(|seq| {
                seq.iter()
                    .filter_map(|p| {
                        if let Some(s) = p.as_str() {
                            Some(s.trim_start_matches("./").to_string())
                        } else {
                            p["name"].as_str().map(String::from)
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    let manifest_before: YamlValue =
        serde_yaml::from_str(&fs::read_to_string(root.join("minds.yaml")).unwrap()).unwrap();
    let names_before = project_ids(&manifest_before);
    assert!(names_before.contains(&"alpha".to_string()), "pre: alpha missing: {names_before:?}");
    assert!(names_before.contains(&"beta".to_string()), "pre: beta missing: {names_before:?}");

    let (stdout, stderr, code) = run_rooted(&repo, &["project", "remove", "alpha", "--yes"]);
    assert_eq!(code, 0, "remove failed\nstdout: {stdout}\nstderr: {stderr}");

    // Post-condition #1: the directory is gone
    assert!(!root.join("alpha").exists(), "alpha/ should be removed");

    // Post-condition #2: minds.yaml no longer references alpha — the consistency
    // check the original e2e suite never made.
    let post_text = fs::read_to_string(root.join("minds.yaml")).unwrap();
    let manifest_after: YamlValue = serde_yaml::from_str(&post_text).unwrap();
    let names_after = project_ids(&manifest_after);
    assert!(
        !names_after.contains(&"alpha".to_string()),
        "minds.yaml still references alpha after remove: {names_after:?}\nmanifest:\n{post_text}"
    );
    assert!(
        names_after.contains(&"beta".to_string()),
        "beta should still be in manifest after removing alpha: got {names_after:?}\nmanifest:\n{post_text}"
    );
}

#[test]
fn e2e_boundary_cjk_asset_identity_roundtrip() {
    let repo = cjk_asset_fixture();

    // 1. list --json: CJK asset path must appear verbatim in JSON identity
    let args = ["--project", "alpha", "--json", "asset", "list"];
    let (stdout, stderr, code) = run_rooted(&repo, &args);
    assert_eq!(code, 0, "list failed\nstdout: {stdout}\nstderr: {stderr}");
    let data = decode_data(&stdout);
    let assets = data
        .get("assets")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("data.assets missing or not array: raw stdout = {stdout}"));
    let cjk_identity = assets
        .iter()
        .find_map(|a| {
            let id = a["identity"].as_str()?;
            if id.contains("封面图") {
                Some(id.to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| panic!("CJK asset missing from list: {stdout}"));
    assert_eq!(cjk_identity, "assets/资料/封面图.png", "identity must equal exact relative path");

    // 2. show with the CJK identity must succeed and echo it. The unified
    // show contract (US2 / contracts/show-layout.md) requires
    // `data: { kind: "asset", identity: <path>, ... }`. Bug surface: asset
    // show currently returns the raw Asset model without `kind` or `identity`.
    let (stdout, stderr, code) = run_rooted(&repo, &["--project", "alpha", "asset", "show", "--json", &cjk_identity]);
    assert_eq!(code, 0, "show {cjk_identity} failed\nstdout: {stdout}\nstderr: {stderr}");
    let data = decode_data(&stdout);
    assert!(data.contains_key("kind"), "asset show data missing `kind` field — spec contract violation: {stdout}");
    assert_eq!(
        data.get("identity").and_then(|v| v.as_str()),
        Some(cjk_identity.as_str()),
        "asset show data missing/wrong `identity` field — spec contract violation: {stdout}"
    );

    // 3. text list: pipe-friendly output must contain the raw CJK bytes
    let (stdout, _, code) = run_rooted(&repo, &["--project", "alpha", "asset", "list"]);
    assert_eq!(code, 0, "text list failed: {stdout}");
    assert!(stdout.contains("资料/封面图.png"), "text list missing CJK path: {stdout}");
}
