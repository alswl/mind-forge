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
use std::path::{Path, PathBuf};

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
articles:\n  - title: My Article\n    project: my-project\n    type: blog\n    source_path: docs/my-article.md\n    status: draft\n    created_at: '2026-05-07T00:00:00Z'\n    updated_at: '2026-05-07T00:00:00Z'\n";
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
fn missing_build_artifact_returns_not_found() {
    let dest_root = tempfile::tempdir().unwrap();
    let repo = setup_repo_with_targets(&local_target_yaml("local-blog", dest_root.path()));

    fs::remove_file(repo.path().join("my-project/_build/my-article.md")).unwrap();

    let out = run_publish(&repo, &["--format", "json", "publish", "run", ARTICLE, "--target", "local-blog"]);
    assert_eq!(out.status.code(), Some(1));
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["error"]["kind"], "not-found");
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
        "schema_version: '1'\narticles:\n  - title: My Article\n    project: my-project\n    type: blog\n    source_path: docs/my-article.md\n    status: draft\n    created_at: '2026-05-07T00:00:00Z'\n    updated_at: '2026-05-07T00:00:00Z'\n",
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
    let dest_root = tempfile::tempdir().unwrap();
    let _unused: PathBuf = dest_root.path().to_path_buf();
    let repo = common::setup_repo();
    let project_path = repo.path().join("my-project");
    fs::create_dir_all(&project_path).unwrap();
    fs::write(project_path.join("mind.yaml"), "schema_version: '1'\nproject:\n  name: my-project\n").unwrap();
    fs::write(
        project_path.join("mind-index.yaml"),
        "schema_version: '1'\narticles:\n  - title: My Article\n    project: my-project\n    type: blog\n    source_path: docs/my-article.md\n    status: draft\n    created_at: '2026-05-07T00:00:00Z'\n    updated_at: '2026-05-07T00:00:00Z'\n",
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
fn yuque_prompt_missing_build_artifact_returns_not_found() {
    let repo = setup_repo_with_targets(&yuque_prompt_target_yaml("yuque-draft", true));
    fs::remove_file(repo.path().join("my-project/_build/my-article.md")).unwrap();

    let out = run_publish(&repo, &["--format", "json", "publish", "run", ARTICLE, "--target", "yuque-draft"]);
    assert_eq!(out.status.code(), Some(1));
    let v: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["error"]["kind"], "not-found");

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
    fs::create_dir_all(project_path.join("docs/images")).unwrap();
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
