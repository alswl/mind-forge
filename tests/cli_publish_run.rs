//! Integration tests for `mf publish run` (feature 009-publish-mvp).
//!
//! Covers US1 (local target), US2 (yuque-prompt target), US4 (not-implemented), and
//! the quickstart end-to-end scenario (SC-004).
//!
//! Success Criteria → Tests:
//!   SC-004 (full lifecycle all exit 0): `quickstart_scenario_a_e2e`
//!   SC-005 (single JSON object): `local_happy_path_json_output`, `yuque_prompt_json_is_single_object`
//!   SC-006 (exit 64 for unsupported types): `not_implemented_target_returns_exit_64_*`
//!   SC-008 (`mind-index.yaml` unchanged by `publish run`): `publish_run_does_not_modify_index`,
//!          `local_dry_run_does_not_write`, `yuque_prompt_does_not_modify_index`

use std::fs;
use std::path::Path;

use assert_cmd::Command;

mod common;

const ARTICLE: &str = "my-article";
const ARTICLE_BODY: &[u8] = b"# Hello\n\nbody bytes.\n";

/// Fixture: Mind Repo with one project, one article in the index, one build artifact.
/// The caller passes any number of `mind.yaml` `publish.targets[]` entries (raw YAML body).
fn setup_repo_with_targets(targets_yaml: &str) -> common::TempDir {
    let repo = common::setup_repo();
    let project_name = "my-project";
    let project_path = repo.path().join(project_name);
    fs::create_dir_all(&project_path).unwrap();

    let mind_yaml = format!(
        "schema_version: '1'\n\
project:\n  name: {project_name}\n\
build:\n  output_dir: _build\n  format: md\n\
publish:\n  targets:\n{targets_yaml}",
    );
    fs::write(project_path.join("mind.yaml"), mind_yaml).unwrap();

    let index_yaml = "schema_version: '1'\n\
articles:\n  - title: My Article\n    project: my-project\n    type: blog\n    article_path: docs/my-article.md\n    status: draft\n    created_at: '2026-05-07T00:00:00Z'\n    updated_at: '2026-05-07T00:00:00Z'\n";
    fs::write(project_path.join("mind-index.yaml"), index_yaml).unwrap();

    fs::create_dir_all(project_path.join("docs")).unwrap();
    fs::write(project_path.join("docs/my-article.md"), ARTICLE_BODY).unwrap();

    fs::create_dir_all(project_path.join("_build")).unwrap();
    fs::write(project_path.join("_build/my-article.md"), ARTICLE_BODY).unwrap();

    repo
}

fn local_target_yaml(name: &str, dest: &Path) -> String {
    format!(
        "    - name: {name}\n      type: local\n      enabled: true\n      config:\n        path: {dest}\n",
        dest = dest.display()
    )
}

fn run_publish(repo: &common::TempDir, args: &[&str]) -> std::process::Output {
    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path().join("my-project"))
        .args(args)
        .output()
        .expect("command runs")
}

fn read_index_bytes(repo: &common::TempDir) -> Vec<u8> {
    fs::read(repo.path().join("my-project/mind-index.yaml")).unwrap()
}

// ---------------------------------------------------------------------------
// US2 — file-based publisher from .mind-forge/publisher/*.yaml
// ---------------------------------------------------------------------------

#[test]
fn file_based_local_publisher_text_output() {
    let dest_root = tempfile::tempdir().unwrap();
    let repo = setup_repo_with_targets(""); // no mind.yaml targets
    common::write_publisher_yaml(
        &repo,
        "blog",
        &format!("type: local\nenabled: true\nconfig:\n  path: {}\n", dest_root.path().display()),
    );

    let out = run_publish(&repo, &["publish", "run", ARTICLE, "--target", "blog"]);
    assert_eq!(out.status.code(), Some(0), "exit 0 expected; stderr={}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).unwrap();

    assert!(stdout.contains("target      blog"), "stdout missing target: {stdout}");
    assert!(stdout.contains("type        local"), "stdout missing type: {stdout}");
    assert!(stdout.contains(&format!("size        {} bytes", ARTICLE_BODY.len())), "stdout missing size: {stdout}");

    let dest_file = dest_root.path().join("my-article.md");
    assert!(dest_file.exists(), "destination file must exist");
    assert_eq!(fs::read(&dest_file).unwrap(), ARTICLE_BODY, "destination file content mismatch");
}

#[test]
fn file_based_local_publisher_json_output() {
    let dest_root = tempfile::tempdir().unwrap();
    let repo = setup_repo_with_targets("");
    common::write_publisher_yaml(
        &repo,
        "blog",
        &format!("type: local\nenabled: true\nconfig:\n  path: {}\n", dest_root.path().display()),
    );

    let out = run_publish(&repo, &["--format", "json", "publish", "run", ARTICLE, "--target", "blog"]);
    assert_eq!(out.status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["status"], "ok");
    let data = &v["data"];
    assert_eq!(data["target_name"], "blog");
    assert_eq!(data["target_type"], "local");
    assert_eq!(data["dry_run"], false);
    assert!(data["destination"].is_string());
}

#[test]
fn file_based_publisher_dry_run() {
    let dest_root = tempfile::tempdir().unwrap();
    let repo = setup_repo_with_targets("");
    common::write_publisher_yaml(
        &repo,
        "blog",
        &format!("type: local\nenabled: true\nconfig:\n  path: {}\n", dest_root.path().display()),
    );

    let out = run_publish(&repo, &["--format", "json", "publish", "run", ARTICLE, "--target", "blog", "--dry-run"]);
    assert_eq!(out.status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["data"]["dry_run"], true);
    assert!(!dest_root.path().join("my-article.md").exists(), "destination must NOT exist after dry-run");
}

#[test]
fn mind_yaml_fallback_when_no_publisher_dir() {
    // T027: No .mind-forge/publisher/ dir, existing mind.yaml targets still work
    let dest_root = tempfile::tempdir().unwrap();
    let repo = setup_repo_with_targets(&local_target_yaml("local-blog", dest_root.path()));

    let out = run_publish(&repo, &["publish", "run", ARTICLE, "--target", "local-blog"]);
    assert_eq!(out.status.code(), Some(0), "mind.yaml fallback must still work");
    assert!(dest_root.path().join("my-article.md").exists(), "destination written via mind.yaml target");
}

#[test]
fn unknown_publisher_returns_not_found() {
    // T028: Unknown publisher is rejected before any publish attempt
    let dest_root = tempfile::tempdir().unwrap();
    let repo = setup_repo_with_targets(&local_target_yaml("existing-target", dest_root.path()));

    let out = run_publish(&repo, &["--format", "json", "publish", "run", ARTICLE, "--target", "does-not-exist"]);
    assert_eq!(out.status.code(), Some(1));
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["error"]["kind"], "not-found");
    assert!(
        v["error"]["message"].as_str().unwrap_or("").contains("does-not-exist"),
        "message should mention the requested name"
    );

    let mut entries = fs::read_dir(dest_root.path()).unwrap();
    assert!(entries.next().is_none(), "no files written on unknown target");
}

#[test]
fn repo_wide_publisher_wins_over_mind_yaml_same_name() {
    // T029: When a repo-wide publisher and mind.yaml target share a name,
    // the repo-wide publisher is used for explicit --target
    let dest_root = tempfile::tempdir().unwrap();
    let other_dest = tempfile::tempdir().unwrap();

    // Create a mind.yaml target with the same name but different destination
    let repo = setup_repo_with_targets(&local_target_yaml("blog", other_dest.path()));

    // Create a repo-wide publisher with the same name
    common::write_publisher_yaml(
        &repo,
        "blog",
        &format!("type: local\nenabled: true\nconfig:\n  path: {}\n", dest_root.path().display()),
    );

    let out = run_publish(&repo, &["publish", "run", ARTICLE, "--target", "blog"]);
    assert_eq!(out.status.code(), Some(0));

    // Repo-wide publisher's destination should be used
    assert!(dest_root.path().join("my-article.md").exists(), "repo-wide publisher destination should be used");
    assert!(!other_dest.path().join("my-article.md").exists(), "mind.yaml target destination should NOT be used");
}

#[test]
fn relative_path_resolves_from_repo_root() {
    // T030: relative config.path in publisher file resolves from repo root (minds.yaml sibling)
    let repo = setup_repo_with_targets("");

    // Use a relative path: should resolve from repo root
    let relative_dest = "publisher-output";
    common::write_publisher_yaml(
        &repo,
        "blog",
        &format!("type: local\nenabled: true\nconfig:\n  path: {relative_dest}\n"),
    );

    // Build artifact is in project-path/_build, but command runs from project dir
    let out = run_publish(&repo, &["publish", "run", ARTICLE, "--target", "blog"]);
    assert_eq!(out.status.code(), Some(0));

    // The destination should be at repo-root/publisher-output/my-article.md
    let dest_file = repo.path().join(relative_dest).join("my-article.md");
    assert!(dest_file.exists(), "relative path should resolve from repo root: {dest_file:?}");
    assert_eq!(fs::read(&dest_file).unwrap(), ARTICLE_BODY);
}

// ---------------------------------------------------------------------------
// T041: Publish-time rejection for invalid file-based publishers
// ---------------------------------------------------------------------------

#[test]
fn disabled_file_based_publisher_rejected_at_publish_time() {
    let dest_root = tempfile::tempdir().unwrap();
    let repo = setup_repo_with_targets("");
    common::write_publisher_yaml(
        &repo,
        "offline",
        &format!("type: local\nenabled: false\nconfig:\n  path: {}\n", dest_root.path().display()),
    );

    let out = run_publish(&repo, &["--format", "json", "publish", "run", ARTICLE, "--target", "offline"]);
    assert_eq!(out.status.code(), Some(2));
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["error"]["kind"], "usage");
    assert!(
        v["error"]["message"].as_str().unwrap_or("").contains("disabled"),
        "message should mention disabled: {}",
        v["error"]["message"]
    );
    assert!(!dest_root.path().join("my-article.md").exists(), "no file written on disabled publisher");
}

#[test]
fn duplicate_file_based_publisher_rejected_at_publish_time() {
    let dest_root = tempfile::tempdir().unwrap();
    let repo = setup_repo_with_targets("");
    // Both files define a publisher named "blog" — different filename stems but same resolved name
    common::write_publishers(
        &repo,
        &[
            (
                "blog-a",
                &format!("name: blog\ntype: local\nenabled: true\nconfig:\n  path: {}\n", dest_root.path().display()),
            ),
            (
                "blog-b",
                &format!("name: blog\ntype: local\nenabled: true\nconfig:\n  path: {}\n", dest_root.path().display()),
            ),
        ],
    );

    let out = run_publish(&repo, &["--format", "json", "publish", "run", ARTICLE, "--target", "blog"]);
    assert_eq!(out.status.code(), Some(2));
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["error"]["kind"], "usage");
}

#[test]
fn secret_field_publisher_rejected_at_publish_time() {
    let dest_root = tempfile::tempdir().unwrap();
    let repo = setup_repo_with_targets("");
    common::write_publisher_yaml(
        &repo,
        "leaky",
        &format!("type: local\nenabled: true\nconfig:\n  path: {}\n  token: sk-123\n", dest_root.path().display()),
    );

    let out = run_publish(&repo, &["--format", "json", "publish", "run", ARTICLE, "--target", "leaky"]);
    assert_eq!(out.status.code(), Some(2));
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["error"]["kind"], "usage");
    assert!(!dest_root.path().join("my-article.md").exists(), "no file written on secret-field publisher");
}

#[test]
fn invalid_name_publisher_rejected_at_publish_time() {
    let _dest_root = tempfile::tempdir().unwrap();
    let repo = setup_repo_with_targets("");
    common::write_publisher_yaml(
        &repo,
        "BadName",
        "name: BadName\ntype: local\nenabled: true\nconfig:\n  path: ./out\n",
    );

    let out = run_publish(&repo, &["--format", "json", "publish", "run", ARTICLE, "--target", "BadName"]);
    assert_eq!(out.status.code(), Some(2));
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["error"]["kind"], "usage");
}

// ---------------------------------------------------------------------------
// US1 — local target
// ---------------------------------------------------------------------------

#[test]
fn local_happy_path_text_output() {
    let dest_root = tempfile::tempdir().unwrap();
    let repo = setup_repo_with_targets(&local_target_yaml("local-blog", dest_root.path()));

    let out = run_publish(&repo, &["publish", "run", ARTICLE, "--target", "local-blog"]);
    let stdout = String::from_utf8(out.stdout.clone()).unwrap();
    let stderr = String::from_utf8(out.stderr.clone()).unwrap();
    assert_eq!(out.status.code(), Some(0), "expected exit 0; stderr={stderr}; stdout={stdout}");

    assert!(stdout.contains("target      local-blog"), "stdout missing target row: {stdout}");
    assert!(stdout.contains("type        local"), "stdout missing type row: {stdout}");
    assert!(stdout.contains("article     my-article"), "stdout missing article row: {stdout}");
    assert!(stdout.contains(&format!("size        {} bytes", ARTICLE_BODY.len())), "stdout missing size row: {stdout}");
    let dest_file = dest_root.path().join("my-article.md");
    assert!(
        stdout.contains(&format!("destination {}", dest_file.display())),
        "stdout missing destination row: {stdout}"
    );

    let written = fs::read(&dest_file).unwrap();
    assert_eq!(written, ARTICLE_BODY, "destination bytes differ from build artifact");
}

#[test]
fn local_happy_path_json_output() {
    let dest_root = tempfile::tempdir().unwrap();
    let repo = setup_repo_with_targets(&local_target_yaml("local-blog", dest_root.path()));

    let out = run_publish(&repo, &["--format", "json", "publish", "run", ARTICLE, "--target", "local-blog"]);
    assert_eq!(out.status.code(), Some(0));

    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("stdout must be a single JSON object (SC-005)");
    assert_eq!(v["status"], "ok");
    let data = &v["data"];
    assert_eq!(data["target_type"], "local");
    assert_eq!(data["target_name"], "local-blog");
    assert_eq!(data["article"], ARTICLE);
    assert_eq!(data["size_bytes"], ARTICLE_BODY.len() as u64);
    assert_eq!(data["dry_run"], false);
    assert!(data["source"].is_string());
    assert!(data["destination"].is_string());
}

#[test]
fn local_dry_run_does_not_write() {
    let dest_root = tempfile::tempdir().unwrap();
    let repo = setup_repo_with_targets(&local_target_yaml("local-blog", dest_root.path()));
    let dest_file = dest_root.path().join("my-article.md");
    let index_before = read_index_bytes(&repo);

    let out =
        run_publish(&repo, &["--format", "json", "publish", "run", ARTICLE, "--target", "local-blog", "--dry-run"]);
    assert_eq!(out.status.code(), Some(0));

    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["data"]["dry_run"], true);

    assert!(!dest_file.exists(), "destination file must NOT exist after dry-run");
    assert_eq!(read_index_bytes(&repo), index_before, "mind-index.yaml must be unchanged (SC-008)");
}

#[test]
fn local_rejects_existing_file_without_force() {
    let dest_root = tempfile::tempdir().unwrap();
    let repo = setup_repo_with_targets(&local_target_yaml("local-blog", dest_root.path()));
    let dest_file = dest_root.path().join("my-article.md");
    fs::write(&dest_file, b"PRE-EXISTING").unwrap();

    let out = run_publish(&repo, &["--format", "json", "publish", "run", ARTICLE, "--target", "local-blog"]);
    assert_eq!(out.status.code(), Some(1));
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["error"]["kind"], "file-exists");
    assert!(v["error"]["hint"].as_str().unwrap_or("").contains("--force"), "hint should mention --force: {v}");

    assert_eq!(fs::read(&dest_file).unwrap(), b"PRE-EXISTING", "destination bytes preserved");
}

#[test]
fn local_force_overwrites_existing_file() {
    let dest_root = tempfile::tempdir().unwrap();
    let repo = setup_repo_with_targets(&local_target_yaml("local-blog", dest_root.path()));
    let dest_file = dest_root.path().join("my-article.md");
    fs::write(&dest_file, b"PRE-EXISTING").unwrap();

    let out = run_publish(&repo, &["publish", "run", ARTICLE, "--target", "local-blog", "--force"]);
    assert_eq!(out.status.code(), Some(0));

    assert_eq!(fs::read(&dest_file).unwrap(), ARTICLE_BODY);

    for entry in fs::read_dir(dest_root.path()).unwrap() {
        let name = entry.unwrap().file_name().to_string_lossy().to_string();
        assert!(!name.contains(".tmp."), "atomic write should not leave a .tmp.* file behind; found {name}");
    }
}

#[test]
fn local_creates_missing_destination_dir() {
    let dest_root = tempfile::tempdir().unwrap();
    let nested = dest_root.path().join("a/b/c");
    let repo = setup_repo_with_targets(&local_target_yaml("local-blog", &nested));

    let out = run_publish(&repo, &["publish", "run", ARTICLE, "--target", "local-blog"]);
    assert_eq!(out.status.code(), Some(0));

    assert!(nested.is_dir(), "nested destination dir must be created");
    assert!(nested.join("my-article.md").exists(), "file must exist in nested dir");
}

#[test]
fn missing_build_artifact_returns_build_artifact_missing() {
    let dest_root = tempfile::tempdir().unwrap();
    let repo = setup_repo_with_targets(&local_target_yaml("local-blog", dest_root.path()));

    fs::remove_file(repo.path().join("my-project/_build/my-article.md")).unwrap();

    let out = run_publish(&repo, &["--format", "json", "publish", "run", ARTICLE, "--target", "local-blog"]);
    assert_eq!(out.status.code(), Some(1));
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["error"]["kind"], "build_artifact_missing");
    assert!(v["error"]["hint"].as_str().unwrap_or("").contains("mf build"), "hint should mention `mf build`: {v}");

    assert!(!dest_root.path().join("my-article.md").exists(), "destination must not be created when artifact missing");
}

#[test]
fn unknown_target_returns_not_found() {
    let dest_root = tempfile::tempdir().unwrap();
    let repo = setup_repo_with_targets(&local_target_yaml("local-blog", dest_root.path()));

    let out = run_publish(&repo, &["--format", "json", "publish", "run", ARTICLE, "--target", "does-not-exist"]);
    assert_eq!(out.status.code(), Some(1));
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["error"]["kind"], "not-found");

    let mut entries = fs::read_dir(dest_root.path()).unwrap();
    assert!(entries.next().is_none(), "no files written outside the repo on failure");
}

#[test]
fn disabled_target_returns_usage() {
    let dest_root = tempfile::tempdir().unwrap();
    let mut yaml = local_target_yaml("local-blog", dest_root.path());
    yaml = yaml.replace("enabled: true", "enabled: false");
    let repo = setup_repo_with_targets(&yaml);

    let out = run_publish(&repo, &["--format", "json", "publish", "run", ARTICLE, "--target", "local-blog"]);
    assert_eq!(out.status.code(), Some(2));
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["error"]["kind"], "usage");

    let mut entries = fs::read_dir(dest_root.path()).unwrap();
    assert!(entries.next().is_none());
}

#[test]
fn default_target_used_when_flag_omitted() {
    let dest_root = tempfile::tempdir().unwrap();
    let project_yaml = format!(
        "schema_version: '1'\nproject:\n  name: my-project\nbuild:\n  output_dir: _build\n  format: md\npublish:\n  default_target: local-blog\n  targets:\n{tgt}",
        tgt = local_target_yaml("local-blog", dest_root.path())
    );
    let repo = common::setup_repo();
    let project_path = repo.path().join("my-project");
    fs::create_dir_all(&project_path).unwrap();
    fs::write(project_path.join("mind.yaml"), project_yaml).unwrap();
    fs::write(
        project_path.join("mind-index.yaml"),
        "schema_version: '1'\narticles:\n  - title: My Article\n    project: my-project\n    type: blog\n    article_path: docs/my-article.md\n    status: draft\n    created_at: '2026-05-07T00:00:00Z'\n    updated_at: '2026-05-07T00:00:00Z'\n",
    )
    .unwrap();
    fs::create_dir_all(project_path.join("docs")).unwrap();
    fs::write(project_path.join("docs/my-article.md"), ARTICLE_BODY).unwrap();
    fs::create_dir_all(project_path.join("_build")).unwrap();
    fs::write(project_path.join("_build/my-article.md"), ARTICLE_BODY).unwrap();

    let out = run_publish(&repo, &["publish", "run", ARTICLE]);
    assert_eq!(out.status.code(), Some(0));
    assert!(dest_root.path().join("my-article.md").exists(), "default target should have produced output");
}

#[test]
fn no_target_no_default_returns_usage() {
    let repo = common::setup_repo();
    let project_path = repo.path().join("my-project");
    fs::create_dir_all(&project_path).unwrap();
    fs::write(project_path.join("mind.yaml"), "schema_version: '1'\nproject:\n  name: my-project\n").unwrap();
    fs::write(
        project_path.join("mind-index.yaml"),
        "schema_version: '1'\narticles:\n  - title: My Article\n    project: my-project\n    type: blog\n    article_path: docs/my-article.md\n    status: draft\n    created_at: '2026-05-07T00:00:00Z'\n    updated_at: '2026-05-07T00:00:00Z'\n",
    )
    .unwrap();
    fs::create_dir_all(project_path.join("_build")).unwrap();
    fs::write(project_path.join("_build/my-article.md"), ARTICLE_BODY).unwrap();

    let out = run_publish(&repo, &["--format", "json", "publish", "run", ARTICLE]);
    assert_eq!(out.status.code(), Some(2));
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["error"]["kind"], "usage");
    assert!(
        v["error"]["hint"].as_str().unwrap_or("").contains("default_target"),
        "hint should mention `default_target`: {v}"
    );
}

#[test]
fn publish_run_does_not_modify_index() {
    let dest_root = tempfile::tempdir().unwrap();
    let repo = setup_repo_with_targets(&local_target_yaml("local-blog", dest_root.path()));

    let index_before = read_index_bytes(&repo);
    let out = run_publish(&repo, &["publish", "run", ARTICLE, "--target", "local-blog"]);
    assert_eq!(out.status.code(), Some(0));
    let index_after = read_index_bytes(&repo);
    assert_eq!(index_before, index_after, "mind-index.yaml MUST be byte-identical (SC-008)");
}

// ---------------------------------------------------------------------------
// US2 — yuque-prompt target
// ---------------------------------------------------------------------------

fn yuque_prompt_target_yaml(name: &str, with_config: bool) -> String {
    if with_config {
        format!(
            "    - name: {name}\n      type: yuque-prompt\n      enabled: true\n      config:\n        namespace: my-blog\n        tags:\n          - tech\n          - rust\n",
        )
    } else {
        format!("    - name: {name}\n      type: yuque-prompt\n      enabled: true\n",)
    }
}

#[test]
fn yuque_prompt_text_two_section_layout() {
    let repo = setup_repo_with_targets(&yuque_prompt_target_yaml("yuque-draft", true));
    let out = run_publish(&repo, &["publish", "run", ARTICLE, "--target", "yuque-draft"]);
    assert_eq!(out.status.code(), Some(0));

    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("### Publish Prompt"), "missing prompt section: {stdout}");
    assert!(stdout.contains("### Envelope"), "missing envelope section: {stdout}");
    assert!(stdout.contains("```json"), "missing json fence: {stdout}");

    let prompt_idx = stdout.find("### Publish Prompt").unwrap();
    let envelope_idx = stdout.find("### Envelope").unwrap();
    let between = &stdout[prompt_idx..envelope_idx];
    assert!(between.contains("\n\n### Envelope") || envelope_idx > prompt_idx, "sections must be separated");

    assert!(!stdout.contains('\u{1b}'), "no ANSI escapes allowed: {stdout}");
}

#[test]
fn yuque_prompt_json_is_single_object() {
    let repo = setup_repo_with_targets(&yuque_prompt_target_yaml("yuque-draft", true));
    let out = run_publish(&repo, &["--format", "json", "publish", "run", ARTICLE, "--target", "yuque-draft"]);
    assert_eq!(out.status.code(), Some(0));

    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("stdout must be a single JSON object (SC-005)");
    assert!(v.is_object(), "must be exactly one JSON object");
    assert_eq!(v["status"], "ok");
    let data = &v["data"];
    assert_eq!(data["target_type"], "yuque-prompt");
    assert_eq!(data["target_name"], "yuque-draft");
    assert_eq!(data["article"], ARTICLE);
    assert_eq!(data["content"].as_str().unwrap().as_bytes(), ARTICLE_BODY, "content must equal build artifact bytes");
    assert!(data["envelope"].is_object());
    assert_eq!(data["envelope"]["namespace"], "my-blog");
    let suggested = data["suggested_update_command"].as_str().unwrap();
    assert!(suggested.starts_with(&format!("mf publish update {ARTICLE} --target yuque-draft")));
}

#[test]
fn yuque_prompt_dry_run_marks_envelope() {
    let repo = setup_repo_with_targets(&yuque_prompt_target_yaml("yuque-draft", true));
    let index_before = read_index_bytes(&repo);

    let out =
        run_publish(&repo, &["--format", "json", "publish", "run", ARTICLE, "--target", "yuque-draft", "--dry-run"]);
    assert_eq!(out.status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["data"]["dry_run"], true);

    assert_eq!(read_index_bytes(&repo), index_before, "mind-index.yaml unchanged");
}

#[test]
fn yuque_prompt_empty_config_defaults_to_object() {
    let repo = setup_repo_with_targets(&yuque_prompt_target_yaml("yuque-draft", false));
    let out = run_publish(&repo, &["--format", "json", "publish", "run", ARTICLE, "--target", "yuque-draft"]);
    assert_eq!(out.status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["data"]["envelope"], serde_json::json!({}));
}

#[test]
fn yuque_prompt_missing_build_artifact_returns_build_artifact_missing() {
    let repo = setup_repo_with_targets(&yuque_prompt_target_yaml("yuque-draft", true));
    fs::remove_file(repo.path().join("my-project/_build/my-article.md")).unwrap();

    let out = run_publish(&repo, &["--format", "json", "publish", "run", ARTICLE, "--target", "yuque-draft"]);
    assert_eq!(out.status.code(), Some(1));
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["error"]["kind"], "build_artifact_missing");

    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.is_empty(), "no stdout output on error: {stdout}");
}

#[test]
fn yuque_prompt_does_not_modify_index() {
    let repo = setup_repo_with_targets(&yuque_prompt_target_yaml("yuque-draft", true));
    let index_before = read_index_bytes(&repo);

    let out = run_publish(&repo, &["publish", "run", ARTICLE, "--target", "yuque-draft"]);
    assert_eq!(out.status.code(), Some(0));

    assert_eq!(read_index_bytes(&repo), index_before, "mind-index.yaml byte-unchanged (SC-008)");
}

// ---------------------------------------------------------------------------
// E2E — quickstart scenario A (SC-004)
// ---------------------------------------------------------------------------

/// Full end-to-end test of quickstart.md Scenario A:
/// `mf build <ARTICLE> && mf publish run <ARTICLE> --target <NAME> && mf publish update <ARTICLE> --target <NAME> --status published --target-url <URL>`
/// All three steps MUST exit 0 (SC-004).
#[test]
fn quickstart_scenario_a_e2e() {
    let repo = common::setup_repo();
    let project_name = "my-blog";
    let project_path = repo.path().join(project_name);
    let article = "hello-world";
    let article_body = b"# Hello World\n\nMy first article.\n";
    let dest_dir = repo.path().join("_site");
    let target_url = format!("file:///{}/hello-world.md", dest_dir.display());

    // --- Setup: project skeleton (simulates `mf project new` + `mf article new`) ---
    fs::create_dir_all(project_path.join("docs")).unwrap();
    fs::write(project_path.join("docs").join(format!("{article}.md")), article_body).unwrap();
    fs::write(
        project_path.join("mind.yaml"),
        format!(
            "schema_version: '1'\n\
             project:\n  name: {project_name}\n\
             build:\n  output_dir: _build\n  format: md\n\
             publish:\n  targets:\n    - name: local-blog\n      type: local\n      enabled: true\n      config:\n        path: {dest}\n",
            dest = dest_dir.display()
        ),
    )
    .unwrap();

    // --- Step 0: `mf article index` to register the article ---
    let out = Command::cargo_bin("mf").unwrap().current_dir(&project_path).args(["article", "index"]).output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "mf article index exited non-zero: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    // --- Step 1: `mf build <ARTICLE>` (must exit 0) ---
    let out = Command::cargo_bin("mf").unwrap().current_dir(&project_path).args(["build", article]).output().unwrap();
    assert_eq!(out.status.code(), Some(0), "mf build exited non-zero: stderr={}", String::from_utf8_lossy(&out.stderr));
    assert!(project_path.join("_build").join(format!("{article}.md")).exists(), "build artifact must exist");

    // --- Step 2: `mf publish run <ARTICLE> --target local-blog` (must exit 0) ---
    let out = Command::cargo_bin("mf")
        .unwrap()
        .current_dir(&project_path)
        .args(["publish", "run", article, "--target", "local-blog"])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "mf publish run exited non-zero: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let dest_file = dest_dir.join(format!("{article}.md"));
    assert!(dest_file.exists(), "publish run must write destination file");
    assert_eq!(
        fs::read(&dest_file).unwrap(),
        article_body,
        "destination file must be byte-identical to build artifact"
    );

    // mind-index.yaml must be unchanged after publish run (SC-008)
    let index_path = project_path.join("mind-index.yaml");

    // --- Step 3: `mf publish update <ARTICLE> --target local-blog --status published --target-url <URL>` ---
    let out = Command::cargo_bin("mf")
        .unwrap()
        .current_dir(&project_path)
        .args([
            "publish",
            "update",
            article,
            "--target",
            "local-blog",
            "--status",
            "published",
            "--target-url",
            &target_url,
        ])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "mf publish update exited non-zero: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Verify publish record was written
    let index_content = fs::read_to_string(&index_path).unwrap();
    assert!(
        index_content.contains("publish_records"),
        "mind-index.yaml must contain publish_records after update; got:\n{index_content}"
    );
    assert!(index_content.contains(&target_url), "mind-index.yaml must contain target_url; got:\n{index_content}");
    assert!(
        index_content.contains("published"),
        "mind-index.yaml must contain status 'published'; got:\n{index_content}"
    );

    // Verify idempotent second update (SC-009)
    let after_first = index_content.clone();
    let out = Command::cargo_bin("mf")
        .unwrap()
        .current_dir(&project_path)
        .args([
            "publish",
            "update",
            article,
            "--target",
            "local-blog",
            "--status",
            "published",
            "--target-url",
            &target_url,
        ])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "second publish update exited non-zero: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let index_after_second = fs::read_to_string(&index_path).unwrap();
    assert_eq!(after_first, index_after_second, "second update must be idempotent (SC-009)");
}

// ---------------------------------------------------------------------------
// US4 — not-implemented target types
// ---------------------------------------------------------------------------

fn not_implemented_target_yaml(name: &str, type_: &str) -> String {
    format!("    - name: {name}\n      type: {type_}\n      enabled: true\n",)
}

fn assert_not_implemented(repo: &common::TempDir, target_name: &str, type_: &str) {
    let index_before = read_index_bytes(repo);

    let out = run_publish(repo, &["--format", "json", "publish", "run", ARTICLE, "--target", target_name]);

    // Must exit 64 per FR-008, FR-304, SC-006
    assert_eq!(out.status.code(), Some(64), "exit code 64 expected for not-implemented target type '{type_}'");

    // Parse JSON error envelope from stderr
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).expect("stderr must be a single JSON object");
    assert_eq!(v["error"]["kind"], "not-implemented", "error kind mismatch for '{type_}'");

    let message = v["error"]["message"].as_str().unwrap_or("");
    assert!(message.contains(type_), "message '{message}' should contain target type '{type_}'");

    let hint = v["error"]["hint"].as_str().unwrap_or("");
    assert!(hint.contains("ROADMAP"), "hint '{hint}' should contain 'ROADMAP' per spec");

    // No stdout output on error
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.is_empty(), "no stdout output on not-implemented error: {stdout}");

    // mind-index.yaml must be unchanged (SC-008)
    assert_eq!(read_index_bytes(repo), index_before, "mind-index.yaml unchanged after not-implemented target (SC-008)");
}

#[test]
fn not_implemented_target_returns_exit_64_yuque() {
    let repo = setup_repo_with_targets(&not_implemented_target_yaml("legacy-yuque", "yuque"));
    assert_not_implemented(&repo, "legacy-yuque", "yuque");
}

#[test]
fn not_implemented_target_returns_exit_64_github_pages() {
    let repo = setup_repo_with_targets(&not_implemented_target_yaml("gh-pages", "github_pages"));
    assert_not_implemented(&repo, "gh-pages", "github_pages");
}

#[test]
fn not_implemented_target_returns_exit_64_custom() {
    let repo = setup_repo_with_targets(&not_implemented_target_yaml("custom-out", "custom"));
    assert_not_implemented(&repo, "custom-out", "custom");
}

// ---------------------------------------------------------------------------
// US1 — publish path expansion and prefix (023 fix-bugs-2 / T008)
// ---------------------------------------------------------------------------

/// Fixture: project with a date-prefixed article and a local target using `path` + `prefix`.
fn setup_us1_project(target_path: &str, prefix: &str, article_name: &str) -> common::TempDir {
    let repo = common::setup_repo();
    let project_name = "my-project";
    let project_path = repo.path().join(project_name);
    fs::create_dir_all(&project_path).unwrap();

    let targets_yaml = format!(
        "    - name: local-out\n      type: local\n      enabled: true\n      path: \"{target_path}\"\n      prefix: \"{prefix}\"\n",
    );
    let mind_yaml = format!(
        "schema_version: '1'\n\
project:\n  name: {project_name}\n\
build:\n  output_dir: _build\n  format: md\n\
publish:\n  targets:\n{targets_yaml}",
    );
    fs::write(project_path.join("mind.yaml"), mind_yaml).unwrap();

    let article_path = format!("docs/{article_name}.md");
    let index_yaml = format!(
        "schema_version: '1'\n\
articles:\n  - title: '{article_name}'\n    project: {project_name}\n    type: blog\n    article_path: {article_path}\n    status: draft\n    created_at: '2026-05-07T00:00:00Z'\n    updated_at: '2026-05-07T00:00:00Z'\n",
    );
    fs::write(project_path.join("mind-index.yaml"), index_yaml).unwrap();

    fs::create_dir_all(project_path.join("docs")).unwrap();
    fs::write(project_path.join(&article_path), ARTICLE_BODY).unwrap();

    let build_artifact = format!("{article_name}.md");
    fs::create_dir_all(project_path.join("_build")).unwrap();
    fs::write(project_path.join("_build").join(&build_artifact), ARTICLE_BODY).unwrap();

    repo
}

const DATED_ARTICLE: &str = "2026-05-15-my-article";

#[test]
fn dry_run_expands_date_placeholder() {
    let repo = setup_us1_project("/tmp/mf-test/{date:YYYY-MM}/daily/", "", DATED_ARTICLE);

    let out = run_publish(
        &repo,
        &["--format", "json", "publish", "run", DATED_ARTICLE, "--target", "local-out", "--dry-run"],
    );
    assert_eq!(out.status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let dest = v["data"]["destination"].as_str().unwrap_or("");
    assert!(dest.contains("/2026-05/daily/"), "destination should contain expanded date '2026-05', got: {dest}");
    assert!(!dest.contains('{'), "destination should not contain literal '{{'");
}

#[test]
fn prefix_applied_to_file_name() {
    let repo = setup_us1_project("/tmp/mf-test/out/", "cie-", DATED_ARTICLE);

    let out = run_publish(
        &repo,
        &["--format", "json", "publish", "run", DATED_ARTICLE, "--target", "local-out", "--dry-run"],
    );
    assert_eq!(out.status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let dest = v["data"]["destination"].as_str().unwrap_or("");
    assert!(dest.contains("cie-2026-05-15-my-article.md"), "destination should contain prefixed filename, got: {dest}");
}

#[test]
fn combined_date_and_prefix() {
    let repo = setup_us1_project("/tmp/mf-test/{date:YYYY-MM}/daily/", "cie-", DATED_ARTICLE);

    let out = run_publish(
        &repo,
        &["--format", "json", "publish", "run", DATED_ARTICLE, "--target", "local-out", "--dry-run"],
    );
    assert_eq!(out.status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let dest = v["data"]["destination"].as_str().unwrap_or("");
    assert!(
        dest.contains("/2026-05/daily/cie-2026-05-15-my-article.md"),
        "destination should have date expansion and prefix, got: {dest}"
    );
}

#[test]
fn unknown_placeholder_errors() {
    let repo = setup_us1_project("/tmp/mf-test/{quarter:QQ}/daily/", "", DATED_ARTICLE);

    let out = run_publish(&repo, &["--format", "json", "publish", "run", DATED_ARTICLE, "--target", "local-out"]);
    assert_eq!(out.status.code(), Some(1));
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["error"]["kind"], "unknown_placeholder");
    assert!(
        v["error"]["hint"].as_str().unwrap_or("").contains("supported placeholders"),
        "hint should list supported placeholders, got: {:?}",
        v["error"]["hint"]
    );
}

#[test]
fn no_effective_date_errors() {
    // Article with no date prefix in filename
    let repo = setup_us1_project("/tmp/mf-test/{date:YYYY-MM}/daily/", "", "plain-article");

    let out = run_publish(&repo, &["--format", "json", "publish", "run", "plain-article", "--target", "local-out"]);
    assert_eq!(out.status.code(), Some(1));
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["error"]["kind"], "no_effective_date");
    assert!(
        v["error"]["hint"].as_str().unwrap_or("").contains("YYYY-MM-DD"),
        "hint should mention adding YYYY-MM-DD prefix, got: {:?}",
        v["error"]["hint"]
    );
}

// ---------------------------------------------------------------------------
// US2 — Declared article publish (FR-002, FR-005)
// ---------------------------------------------------------------------------

fn json_run(args: &[&str], cwd: &std::path::Path) -> (serde_json::Value, String, Option<i32>) {
    let output =
        Command::cargo_bin("mf").expect("binary exists").current_dir(cwd).args(args).output().expect("command runs");
    let code = output.status.code();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let body = if code != Some(0) && !stderr.is_empty() { &stderr } else { &stdout };
    let parsed = serde_json::from_str(body).unwrap_or_else(|_| serde_json::Value::String(body.clone()));
    (parsed, stderr, code)
}

#[test]
fn publish_run_declared_missing_source_returns_no_article_files() {
    let repo = common::setup_repo();
    common::scaffold_three_source_project(&repo, "q24");
    let project_path = repo.path().join("q24");

    // Index first so declared articles are known
    let (_, stderr, code) = json_run(&["article", "index", "-p", "q24"], project_path.as_path());
    assert_eq!(code, Some(0), "index: stderr={stderr}");

    // Try to publish legacy-blog (compat declared, no article content on disk)
    let (parsed, stderr, code) = json_run(
        &[
            "--format",
            "json",
            "publish",
            "run",
            "legacy-blog",
            "--target",
            "simple-out",
            "--project",
            "q24",
            "--dry-run",
        ],
        project_path.as_path(),
    );
    assert_eq!(code, Some(1), "publish of missing-article declared article should fail: {stderr}");
    assert_eq!(
        parsed["error"]["kind"], "no_article_files",
        "FR-005: error.kind should be 'no_article_files', got: {parsed}"
    );
}

#[test]
fn publish_run_declared_present_succeeds_dry_run() {
    let repo = common::setup_repo();
    common::scaffold_three_source_project(&repo, "q24");
    let project_path = repo.path().join("q24");

    // First build the declared article to create its artifact
    let out = Command::cargo_bin("mf").unwrap().current_dir(&project_path).args(["build", "reports"]).output().unwrap();
    assert_eq!(out.status.code(), Some(0), "build reports: stderr={}", String::from_utf8_lossy(&out.stderr));

    // Index
    let (_, stderr, code) = json_run(&["article", "index", "-p", "q24"], project_path.as_path());
    assert_eq!(code, Some(0), "index: stderr={stderr}");

    // Publish the declared article
    let (parsed, stderr, code) = json_run(
        &["--format", "json", "publish", "run", "reports", "--target", "simple-out", "--project", "q24", "--dry-run"],
        project_path.as_path(),
    );
    assert_eq!(code, Some(0), "publish declared should succeed: stderr={stderr} parsed={parsed}");
    let source = parsed["data"]["source"].as_str().unwrap_or("");
    assert!(
        source.ends_with("_build/reports.md"),
        "FR-002: source should be _build/reports.md (article_key-derived), not path-joined from args: {source}"
    );
}

#[test]
fn publish_run_auto_reindex_picks_up_declared_articles() {
    let repo = common::setup_repo();
    common::scaffold_three_source_project(&repo, "q24");
    let project_path = repo.path().join("q24");

    // Build to create the artifact
    let out = Command::cargo_bin("mf").unwrap().current_dir(&project_path).args(["build", "reports"]).output().unwrap();
    assert_eq!(out.status.code(), Some(0), "build reports: stderr={}", String::from_utf8_lossy(&out.stderr));

    // Delete index to trigger auto-reindex
    let index_path = project_path.join("mind-index.yaml");
    let _ = std::fs::remove_file(&index_path);

    // Publish — should auto-reindex and find the declared article
    let (parsed, stderr, code) = json_run(
        &["--format", "json", "publish", "run", "reports", "--target", "simple-out", "--project", "q24", "--dry-run"],
        project_path.as_path(),
    );
    assert_eq!(code, Some(0), "auto-reindex should find declared article: stderr={stderr} parsed={parsed}");
    assert!(index_path.exists(), "FR-008: mind-index.yaml should have been recreated by auto-reindex");
}

// ---------------------------------------------------------------------------
// US3: Publish declared directory article by key (T027)
// ---------------------------------------------------------------------------

#[test]
fn publish_declared_directory_article_dry_run() {
    let dest = tempfile::tempdir().unwrap();
    let repo = common::setup_repo();
    let project_path = repo.path().join("team-reports");
    fs::create_dir_all(project_path.join("docs/2026-05-monthly")).unwrap();
    fs::write(project_path.join("docs/2026-05-monthly/01-team-okr.md"), b"# Monthly\n\nmonthly content\n").unwrap();

    let mind_yaml = format!(
        "schema_version: '1'\n\
         project:\n  name: team-reports\n\
         build:\n  output_dir: _build\n  format: md\n\
           articles:\n    2026-05-monthly: {{}}\n\
         publish:\n  targets:\n    - name: local-out\n      type: local\n      enabled: true\n      path: \"{}\"\n      prefix: \"\"\n",
        dest.path().display()
    );
    fs::write(project_path.join("mind.yaml"), mind_yaml).unwrap();

    // Index
    let (_, stderr, code) = json_run(&["article", "index"], project_path.as_path());
    assert_eq!(code, Some(0), "index: stderr={stderr}");

    // Build to create artifact
    let out = Command::cargo_bin("mf")
        .unwrap()
        .current_dir(&project_path)
        .args(["build", "2026-05-monthly"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "build: stderr={}", String::from_utf8_lossy(&out.stderr));

    // Publish dry-run by key
    let (parsed, stderr, code) = json_run(
        &["--format", "json", "publish", "run", "2026-05-monthly", "--target", "local-out", "--dry-run"],
        project_path.as_path(),
    );
    assert_eq!(code, Some(0), "publish declared dir article should succeed: stderr={stderr}");
    let source = parsed["data"]["source"].as_str().unwrap_or("");
    assert!(source.contains("_build/2026-05-monthly.md"), "source should reference build artifact, got: {source}");
}

// ---------------------------------------------------------------------------
// US3: Regression — title never used in publish error messages (T028)
// ---------------------------------------------------------------------------

#[test]
fn publish_title_not_used_for_error_messages() {
    let dest = tempfile::tempdir().unwrap();
    let repo = common::setup_repo();
    let project_path = repo.path().join("team-reports");
    fs::create_dir_all(project_path.join("docs/2026-05-monthly")).unwrap();
    fs::write(project_path.join("docs/2026-05-monthly/01-team-okr.md"), b"# Monthly\n\nmonthly content\n").unwrap();

    let mind_yaml = format!(
        "schema_version: '1'\n\
         project:\n  name: team-reports\n\
         build:\n  output_dir: _build\n  format: md\n\
           articles:\n    2026-05-monthly: {{}}\n\
         publish:\n  targets:\n    - name: local-out\n      type: local\n      enabled: true\n      path: \"{}\"\n      prefix: \"\"\n",
        dest.path().display()
    );
    fs::write(project_path.join("mind.yaml"), mind_yaml).unwrap();

    // Index
    let (_, stderr, code) = json_run(&["article", "index"], project_path.as_path());
    assert_eq!(code, Some(0), "index: stderr={stderr}");

    // Build
    let out = Command::cargo_bin("mf")
        .unwrap()
        .current_dir(&project_path)
        .args(["build", "2026-05-monthly"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "build: stderr={}", String::from_utf8_lossy(&out.stderr));

    // Publish by key succeeds
    let (parsed, stderr, code) = json_run(
        &["--format", "json", "publish", "run", "2026-05-monthly", "--target", "local-out", "--dry-run"],
        project_path.as_path(),
    );
    assert_eq!(code, Some(0), "publish by key should succeed: stderr={stderr}");
    assert_eq!(parsed["status"], "ok");

    // Try to publish an article that doesn't exist — error should NOT use title
    let (parsed, stderr, code) = json_run(
        &["--format", "json", "publish", "run", "nonexistent", "--target", "local-out", "--dry-run"],
        project_path.as_path(),
    );
    assert_ne!(code, Some(0), "publish of nonexistent article should fail");
    assert_eq!(parsed["error"]["kind"], "not-found");
    let msg = parsed["error"]["message"].as_str().unwrap_or("");
    // The error must NOT mention a kebab→title-space conversion (the old bug)
    assert!(!msg.contains("2026 05 monthly"), "error must not contain display-title: {msg}");
    assert!(!stderr.contains("2026 05 monthly"), "stderr must not contain display-title: {stderr}");
}

// ---------------------------------------------------------------------------
// US3: dry-run writes no destination content (T031)
// ---------------------------------------------------------------------------

#[test]
fn publish_dry_run_does_not_write_destination_file() {
    let dest = tempfile::tempdir().unwrap();
    let repo = common::setup_repo();
    let project_path = repo.path().join("team-reports");
    fs::create_dir_all(project_path.join("docs/2026-05-monthly")).unwrap();
    fs::write(project_path.join("docs/2026-05-monthly/01-team-okr.md"), b"# Monthly\n\ncontent\n").unwrap();

    let mind_yaml = format!(
        "schema_version: '1'\n\
         project:\n  name: team-reports\n\
         build:\n  output_dir: _build\n  format: md\n\
           articles:\n    2026-05-monthly: {{}}\n\
         publish:\n  targets:\n    - name: local-out\n      type: local\n      enabled: true\n      path: \"{}\"\n      prefix: \"\"\n",
        dest.path().display()
    );
    fs::write(project_path.join("mind.yaml"), mind_yaml).unwrap();

    // Index
    let (_, stderr, code) = json_run(&["article", "index"], project_path.as_path());
    assert_eq!(code, Some(0), "index: stderr={stderr}");

    // Build
    let out = Command::cargo_bin("mf")
        .unwrap()
        .current_dir(&project_path)
        .args(["build", "2026-05-monthly"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "build: stderr={}", String::from_utf8_lossy(&out.stderr));

    // Dry-run
    let (parsed, stderr, code) = json_run(
        &["--format", "json", "publish", "run", "2026-05-monthly", "--target", "local-out", "--dry-run"],
        project_path.as_path(),
    );
    assert_eq!(code, Some(0), "dry-run should succeed: stderr={stderr}");
    assert_eq!(parsed["data"]["dry_run"], true);

    // Destination file must NOT exist
    let dest_file = dest.path().join("2026-05-monthly.md");
    assert!(!dest_file.exists(), "dry-run must not write destination file: {dest_file:?}");

    // No files at all in the dest dir
    let entries: Vec<_> = fs::read_dir(dest.path()).unwrap().collect();
    assert!(entries.is_empty(), "dry-run must leave dest dir empty, found {entries:?}");
}

// ---------------------------------------------------------------------------
// BUG-4 reverify — date expansion + prefix works end-to-end (FR-009)
// ---------------------------------------------------------------------------

#[test]
fn publish_run_local_target_expands_date_and_prefix_for_generated() {
    let repo = common::setup_repo();
    common::scaffold_three_source_project(&repo, "q24");
    let project_path = repo.path().join("q24");

    // Index to register generated articles (no build needed — source IS artifact)
    let (_, stderr, code) = json_run(&["article", "index", "-p", "q24"], project_path.as_path());
    assert_eq!(code, Some(0), "index: stderr={stderr}");

    // Publish generated article to paas-git target with date template + prefix
    let (parsed, stderr, code) = json_run(
        &[
            "--format",
            "json",
            "publish",
            "run",
            "daily_report/2026-05-15",
            "--target",
            "paas-git",
            "--project",
            "q24",
            "--dry-run",
        ],
        project_path.as_path(),
    );
    assert_eq!(code, Some(0), "FR-009: generated article publish should succeed: stderr={stderr}");
    let dest = parsed["data"]["destination"].as_str().unwrap_or("");
    assert!(
        dest.contains("/2026-05/daily/cie-2026-05-15.md"),
        "FR-009: destination should expand date and prefix for generated article, got: {dest}"
    );
}

#[test]
fn publish_run_local_target_expands_date_and_prefix_for_docs() {
    let repo = common::setup_repo();
    common::scaffold_three_source_project(&repo, "q24");
    let project_path = repo.path().join("q24");

    // Index first so `mf build` can find the article
    let (_, stderr, code) = json_run(&["article", "index", "-p", "q24"], project_path.as_path());
    assert_eq!(code, Some(0), "index: stderr={stderr}");

    // Build artifact for the docs article
    let out = Command::cargo_bin("mf")
        .unwrap()
        .current_dir(&project_path)
        .args(["build", "2026-05-10-hello"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "build: stderr={}", String::from_utf8_lossy(&out.stderr));

    // Publish docs article to paas-git target with date template + prefix
    let (parsed, stderr, code) = json_run(
        &[
            "--format",
            "json",
            "publish",
            "run",
            "2026-05-10-hello",
            "--target",
            "paas-git",
            "--project",
            "q24",
            "--dry-run",
        ],
        project_path.as_path(),
    );
    assert_eq!(code, Some(0), "FR-009: docs article publish should succeed: stderr={stderr}");
    let dest = parsed["data"]["destination"].as_str().unwrap_or("");
    assert!(
        dest.contains("/2026-05/daily/cie-2026-05-10-hello.md"),
        "FR-009: destination should expand date and prefix for docs article, got: {dest}"
    );
}
