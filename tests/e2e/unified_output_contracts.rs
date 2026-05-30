use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::helpers::*;

fn write_file(path: impl AsRef<Path>, contents: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dir");
    }
    fs::write(path, contents).expect("write fixture file");
}

fn setup_unified_output_repo() -> TempDir {
    let repo = TempDir::new().expect("temp repo");
    let root = repo.path();

    write_file(
        root.join("minds.yaml"),
        r#"schema_version: '1'
projects_dir: '.'
projects:
  - name: alpha
    path: ./alpha
    created_at: '2026-05-29T00:00:00Z'
    archived_at: ~
"#,
    );
    write_file(
        root.join("alpha/mind.yaml"),
        r#"schema_version: '1'
project:
  name: alpha
build:
  output_dir: _build
  format: md
"#,
    );
    write_file(
        root.join("alpha/mind-index.yaml"),
        r#"schema_version: '1'
articles:
  - title: First Post
    project: alpha
    article_type: blog
    article_path: docs/first-post.md
    status: draft
    created_at: '2026-05-29T00:00:00Z'
    updated_at: '2026-05-29T00:00:00Z'
sources:
  - name: paper
    type: pdf
    path: sources/paper.pdf
    tags: []
    added_at: '2026-05-29T00:00:00Z'
    updated_at: '2026-05-29T00:00:00Z'
assets:
  - name: logo.png
    type: image
    path: assets/logo.png
    size: 128
    hash: d34db33f
    tags: []
    added_at: '2026-05-29T00:00:00Z'
terms:
  - term: RAG
    definition: Retrieval-Augmented Generation
    aliases: []
    tags: []
    corrections: []
"#,
    );
    write_file(root.join("alpha/docs/first-post.md"), "# First Post\n");
    write_file(root.join("alpha/sources/paper.pdf"), "paper\n");
    write_file(root.join("alpha/assets/logo.png"), "logo\n");
    write_file(
        root.join(".mind-forge/publisher/local-blog.yaml"),
        r#"type: local
enabled: true
label: Local Blog
config:
  path: ./published
"#,
    );
    write_file(root.join(".mind-forge/renders/brief.md"), "# Brief\n\n{{ content }}\n");

    repo
}

fn root_arg(repo: &TempDir) -> String {
    repo.path().to_string_lossy().into_owned()
}

fn asset_fixture_path(repo: &TempDir) -> PathBuf {
    repo.path().join("alpha/assets/logo.png")
}

fn run_rooted(repo: &TempDir, args: &[&str]) -> (String, String, i32) {
    let root = root_arg(repo);
    let mut full_args = vec!["--root", root.as_str()];
    full_args.extend_from_slice(args);
    run_in(repo.path(), &full_args)
}

fn decode_ok(stdout: &str) -> Value {
    let value: Value = serde_json::from_str(stdout).expect("stdout should be JSON");
    assert_eq!(value["status"], "ok", "unexpected envelope: {stdout}");
    assert_eq!(value["command"], "mf", "unexpected command field: {stdout}");
    value
}

fn data_object(stdout: &str) -> serde_json::Map<String, Value> {
    decode_ok(stdout)["data"].as_object().expect("data should be object").clone()
}

fn assert_success(stdout: &str, stderr: &str, code: i32, args: &[&str]) {
    assert_eq!(code, 0, "mf {args:?} failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
}

#[test]
fn e2e_unified_list_show_identity_roundtrip() {
    let repo = setup_unified_output_repo();

    let cases = [
        ("project", vec!["--json", "project", "list"], "projects", vec!["--json", "project", "show"], "project"),
        (
            "article",
            vec!["--project", "alpha", "--json", "article", "list"],
            "articles",
            vec!["--project", "alpha", "--json", "article", "show"],
            "article",
        ),
        (
            "source",
            vec!["--project", "alpha", "--json", "source", "list"],
            "sources",
            vec!["--project", "alpha", "--json", "source", "show"],
            "source",
        ),
        (
            "asset",
            vec!["--project", "alpha", "--json", "asset", "list"],
            "assets",
            vec!["--project", "alpha", "--json", "asset", "show"],
            "asset",
        ),
        (
            "term",
            vec!["--project", "alpha", "--json", "term", "list"],
            "terms",
            vec!["--project", "alpha", "--json", "term", "show"],
            "term",
        ),
        (
            "publish target",
            vec!["--json", "publish", "target", "list"],
            "publish_targets",
            vec!["--json", "publish", "target", "show"],
            "publish_target",
        ),
        (
            "render template",
            vec!["--json", "render", "template", "list"],
            "templates",
            vec!["--json", "render", "template", "show"],
            "render_template",
        ),
    ];

    for (name, list_args, collection, show_prefix, kind) in cases {
        let (stdout, stderr, code) = run_rooted(&repo, &list_args);
        assert_success(&stdout, &stderr, code, &list_args);
        let list = decode_ok(&stdout);
        let items = list["data"][collection]
            .as_array()
            .unwrap_or_else(|| panic!("{name} list missing data.{collection}: {stdout}"));
        let first = items.first().unwrap_or_else(|| panic!("{name} list returned no fixture rows"));
        let identity = first["identity"].as_str().unwrap_or_else(|| panic!("{name} row missing identity: {stdout}"));

        let mut show_args = show_prefix;
        show_args.push(identity);
        let (stdout, stderr, code) = run_rooted(&repo, &show_args);
        assert_success(&stdout, &stderr, code, &show_args);
        let show = decode_ok(&stdout);
        if let Some(actual_kind) = show["data"]["kind"].as_str() {
            assert_eq!(actual_kind, kind, "{name} show kind mismatch: {stdout}");
            assert_eq!(show["data"]["identity"], identity, "{name} show identity mismatch: {stdout}");
        } else {
            assert!(
                show["data"]["identity"] == identity
                    || show["data"]["name"] == identity
                    || show["data"]["path"] == identity
                    || show["data"]["term"] == identity,
                "{name} show did not echo list identity {identity:?}: {stdout}"
            );
        }
    }
}

#[test]
fn e2e_unified_create_commands_emit_canonical_dry_run_envelopes() {
    let repo = setup_unified_output_repo();
    let asset = asset_fixture_path(&repo);
    let asset = asset.to_string_lossy();

    let cases = [
        (vec!["--json", "project", "new", "preview", "--dry-run"], "project"),
        (vec!["--project", "alpha", "--json", "article", "new", "Preview Article", "--dry-run"], "article"),
        (
            vec![
                "--project",
                "alpha",
                "--json",
                "term",
                "new",
                "LLM",
                "--definition",
                "Large Language Model",
                "--dry-run",
            ],
            "term",
        ),
        (
            vec![
                "--project",
                "alpha",
                "--json",
                "source",
                "add",
                "https://example.com/ref",
                "--name",
                "web-ref",
                "--dry-run",
            ],
            "source",
        ),
        (vec!["--project", "alpha", "--json", "asset", "add", asset.as_ref(), "--dry-run"], "asset"),
        (
            vec!["--project", "alpha", "--json", "term", "add", "--term", "RAG", "--alias", "rag", "--dry-run"],
            "term_correction",
        ),
    ];

    for (args, kind) in cases {
        let (stdout, stderr, code) = run_rooted(&repo, &args);
        assert_success(&stdout, &stderr, code, &args);
        let data = data_object(&stdout);
        assert_eq!(data["kind"], kind, "unexpected kind for {args:?}: {stdout}");
        assert!(data["identity"].as_str().is_some_and(|s| !s.is_empty()), "missing identity: {stdout}");
        assert!(data.contains_key("details"), "missing details: {stdout}");
        assert_eq!(data["dry_run"], true, "dry_run should be true: {stdout}");
    }
}

#[test]
fn e2e_unified_modify_commands_emit_canonical_dry_run_envelopes() {
    let repo = setup_unified_output_repo();

    let rename_cases = [
        (vec!["--json", "project", "rename", "alpha", "beta", "--dry-run"], "project", "alpha", "beta"),
        (
            vec![
                "--project",
                "alpha",
                "--json",
                "article",
                "rename",
                "docs/first-post.md",
                "docs/renamed.md",
                "--dry-run",
            ],
            "article",
            "docs/first-post.md",
            "docs/renamed.md",
        ),
        (
            vec!["--project", "alpha", "--json", "source", "rename", "paper", "paper-renamed", "--dry-run"],
            "source",
            "paper",
            "paper-renamed",
        ),
        (
            vec![
                "--project",
                "alpha",
                "--json",
                "asset",
                "rename",
                "assets/logo.png",
                "assets/logo-renamed.png",
                "--dry-run",
            ],
            "asset",
            "assets/logo.png",
            "assets/logo-renamed.png",
        ),
        (
            vec!["--project", "alpha", "--json", "term", "rename", "RAG", "Retrieval", "--dry-run"],
            "term",
            "RAG",
            "Retrieval",
        ),
    ];

    for (args, kind, old_identity, new_identity) in rename_cases {
        let (stdout, stderr, code) = run_rooted(&repo, &args);
        assert_success(&stdout, &stderr, code, &args);
        let data = data_object(&stdout);
        assert_eq!(data["kind"], kind, "unexpected kind: {stdout}");
        assert_eq!(data["old_identity"], old_identity, "unexpected old identity: {stdout}");
        assert_eq!(data["new_identity"], new_identity, "unexpected new identity: {stdout}");
        assert_eq!(data["dry_run"], true, "dry_run should be true: {stdout}");
    }

    let index_cases = [
        (vec!["--json", "project", "index", "--dry-run"], "project"),
        (vec!["--project", "alpha", "--json", "article", "index", "--dry-run"], "article"),
        (vec!["--project", "alpha", "--json", "source", "index", "--dry-run"], "source"),
        (vec!["--project", "alpha", "--json", "asset", "index", "--dry-run"], "asset"),
    ];

    for (args, kind) in index_cases {
        let (stdout, stderr, code) = run_rooted(&repo, &args);
        assert_success(&stdout, &stderr, code, &args);
        let data = data_object(&stdout);
        assert_eq!(data["kind"], kind, "unexpected kind: {stdout}");
        assert!(data["added"].is_array(), "missing added array: {stdout}");
        assert!(data["removed"].is_array(), "missing removed array: {stdout}");
        assert!(data["kept_count"].is_number(), "missing kept_count: {stdout}");
        assert!(data["scanned_count"].is_number(), "missing scanned_count: {stdout}");
        assert_eq!(data["dry_run"], true, "dry_run should be true: {stdout}");
    }
}

#[test]
fn e2e_unified_json_data_fields_are_objects() {
    let repo = setup_unified_output_repo();
    let asset = asset_fixture_path(&repo);
    let asset = asset.to_string_lossy();

    let commands = [
        vec!["--json", "config", "show"],
        vec!["--json", "version"],
        vec!["--json", "project", "new", "preview", "--dry-run"],
        vec!["--project", "alpha", "--json", "article", "new", "Preview Article", "--dry-run"],
        vec!["--project", "alpha", "--json", "term", "new", "LLM", "--definition", "Large Language Model", "--dry-run"],
        vec![
            "--project",
            "alpha",
            "--json",
            "source",
            "add",
            "https://example.com/ref",
            "--name",
            "web-ref",
            "--dry-run",
        ],
        vec!["--project", "alpha", "--json", "asset", "add", asset.as_ref(), "--dry-run"],
        vec!["--json", "project", "rename", "alpha", "beta", "--dry-run"],
        vec!["--project", "alpha", "--json", "source", "index", "--dry-run"],
        vec!["--project", "alpha", "--json", "asset", "index", "--dry-run"],
        vec!["--project", "alpha", "--json", "article", "index", "--dry-run"],
    ];

    for args in commands {
        let (stdout, stderr, code) = run_rooted(&repo, &args);
        assert_success(&stdout, &stderr, code, &args);
        assert!(decode_ok(&stdout)["data"].is_object(), "data should be object for {args:?}: {stdout}");
    }
}

#[test]
fn e2e_unified_dry_run_does_not_mutate_filesystem() {
    let repo = setup_unified_output_repo();
    let before_manifest = fs::read_to_string(repo.path().join("minds.yaml")).expect("read manifest");
    let before_index = fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).expect("read index");

    let commands = [
        vec!["--json", "project", "new", "preview", "--dry-run"],
        vec!["--json", "project", "rename", "alpha", "beta", "--dry-run"],
        vec!["--project", "alpha", "--json", "term", "new", "LLM", "--definition", "Large Language Model", "--dry-run"],
        vec!["--project", "alpha", "--json", "asset", "index", "--dry-run"],
    ];

    for args in commands {
        let (stdout, stderr, code) = run_rooted(&repo, &args);
        assert_success(&stdout, &stderr, code, &args);
        assert_eq!(decode_ok(&stdout)["data"]["dry_run"], true, "dry_run should be true: {stdout}");
    }

    assert!(!repo.path().join("preview").exists(), "dry-run project new created directory");
    assert_eq!(fs::read_to_string(repo.path().join("minds.yaml")).unwrap(), before_manifest);
    assert_eq!(fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap(), before_index);
}

#[test]
fn e2e_unified_help_exposes_shared_flags() {
    let repo = setup_unified_output_repo();

    let list_commands = [
        vec!["project", "list", "--help"],
        vec!["--project", "alpha", "article", "list", "--help"],
        vec!["--project", "alpha", "source", "list", "--help"],
        vec!["--project", "alpha", "asset", "list", "--help"],
        vec!["--project", "alpha", "term", "list", "--help"],
        vec!["publish", "target", "list", "--help"],
        vec!["render", "template", "list", "--help"],
    ];
    for args in list_commands {
        let (stdout, stderr, code) = run_rooted(&repo, &args);
        assert_success(&stdout, &stderr, code, &args);
        assert!(stdout.contains("--no-headers"), "help missing --no-headers: {stdout}");
        assert!(stdout.contains("--no-trunc"), "help missing --no-trunc: {stdout}");
    }

    let mutating_commands = [
        (vec!["project", "new", "--help"], vec!["--dry-run", "--force"]),
        (vec!["article", "new", "--help"], vec!["--dry-run", "--force"]),
        (vec!["source", "add", "--help"], vec!["--dry-run", "--force"]),
        (vec!["asset", "add", "--help"], vec!["--dry-run", "--force"]),
        (vec!["project", "rename", "--help"], vec!["--dry-run", "--force"]),
        (vec!["article", "rename", "--help"], vec!["--dry-run", "--force"]),
        (vec!["source", "rename", "--help"], vec!["--dry-run", "--force"]),
        (vec!["asset", "rename", "--help"], vec!["--dry-run", "--force"]),
        (vec!["term", "rename", "--help"], vec!["--dry-run", "--force"]),
        (vec!["project", "remove", "--help"], vec!["--dry-run", "--force", "--yes"]),
        (vec!["article", "remove", "--help"], vec!["--dry-run", "--force", "--yes"]),
        (vec!["source", "remove", "--help"], vec!["--dry-run", "--force", "--yes"]),
        (vec!["asset", "remove", "--help"], vec!["--dry-run", "--force", "--yes"]),
        (vec!["term", "remove", "--help"], vec!["--dry-run", "--force", "--yes"]),
        (vec!["project", "archive", "--help"], vec!["--dry-run", "--force", "--yes"]),
    ];
    for (args, flags) in mutating_commands {
        let (stdout, stderr, code) = run_rooted(&repo, &args);
        assert_success(&stdout, &stderr, code, &args);
        for flag in flags {
            assert!(stdout.contains(flag), "help for {args:?} missing {flag}: {stdout}");
        }
    }
}

#[test]
fn e2e_unified_lint_commands_expose_shared_contracts() {
    let repo = setup_unified_output_repo();
    let help_commands = [
        vec!["project", "lint", "--help"],
        vec!["--project", "alpha", "article", "lint", "--help"],
        vec!["--project", "alpha", "term", "lint", "--help"],
    ];
    for args in help_commands {
        let (stdout, stderr, code) = run_rooted(&repo, &args);
        assert_success(&stdout, &stderr, code, &args);
        for flag in ["--fix", "--rule", "--severity", "--max-warnings", "--dry-run"] {
            assert!(stdout.contains(flag), "help for {args:?} missing {flag}: {stdout}");
        }
    }

    let json_commands = [
        vec!["--project", "alpha", "--json", "project", "lint", "--severity", "info", "--max-warnings", "999"],
        vec![
            "--project",
            "alpha",
            "--json",
            "article",
            "lint",
            "--severity",
            "info",
            "--max-warnings",
            "999",
            "--dry-run",
            "--fix",
        ],
        vec![
            "--project",
            "alpha",
            "--json",
            "term",
            "lint",
            "--severity",
            "info",
            "--max-warnings",
            "999",
            "--dry-run",
            "--fix",
        ],
    ];
    for args in json_commands {
        let (stdout, stderr, code) = run_rooted(&repo, &args);
        assert_success(&stdout, &stderr, code, &args);
        let data = data_object(&stdout);
        if let Some(kind) = data.get("kind") {
            assert!(kind.as_str().is_some_and(|s| !s.is_empty()), "empty kind: {stdout}");
        }
        if let Some(issues) = data.get("issues") {
            assert!(issues.is_array(), "issues should be array: {stdout}");
        }
        if let Some(summary) = data.get("summary") {
            assert!(summary.is_object(), "summary should be object: {stdout}");
        }
        assert!(
            data.contains_key("issues") || data.contains_key("findings") || data.contains_key("summary"),
            "lint data should expose machine-readable findings or summary: {stdout}"
        );
        assert!(data.contains_key("dry_run"), "missing dry_run: {stdout}");
    }
}

#[test]
fn e2e_unified_text_output_is_pipe_friendly() {
    let repo = setup_unified_output_repo();
    let cases = [
        (vec!["project", "list"], "NAME", "alpha"),
        (vec!["--project", "alpha", "article", "list"], "PATH", "docs/first-post.md"),
        (vec!["--project", "alpha", "source", "list"], "NAME", "paper"),
        (vec!["--project", "alpha", "asset", "list"], "NAME", "assets/logo.png"),
        (vec!["--project", "alpha", "term", "list"], "TERM", "RAG"),
        (vec!["publish", "target", "list"], "NAME", "local-blog"),
        (vec!["render", "template", "list"], "NAME", "brief"),
    ];

    for (args, header, first_value) in cases {
        let (stdout, stderr, code) = run_rooted(&repo, &args);
        assert_success(&stdout, &stderr, code, &args);
        assert!(stdout.ends_with('\n'), "stdout should end with newline: {stdout:?}");
        assert!(!stdout.contains("\x1b["), "pipe-mode stdout should not contain ANSI: {stdout}");
        let first_line = stdout.trim_end().lines().next().expect("non-empty stdout");
        assert!(
            !first_line.contains(header) || first_line.contains(first_value),
            "pipe-mode first line looks like a header: {first_line:?}\nstdout:\n{stdout}"
        );
        for (line_no, line) in stdout.trim_end_matches('\n').lines().enumerate() {
            assert_eq!(
                line.trim_end_matches([' ', '\t']),
                line,
                "line {} has trailing whitespace: {stdout}",
                line_no + 1
            );
        }
    }
}

#[test]
fn e2e_unified_quiet_suppresses_successful_non_list_output() {
    let repo = setup_unified_output_repo();
    let commands = [
        vec!["--quiet", "project", "show", "alpha"],
        vec!["--quiet", "project", "new", "preview", "--dry-run"],
        vec!["--quiet", "project", "rename", "alpha", "beta", "--dry-run"],
        vec!["--quiet", "--project", "alpha", "asset", "index", "--dry-run"],
    ];

    for args in commands {
        let (stdout, stderr, code) = run_rooted(&repo, &args);
        assert_success(&stdout, &stderr, code, &args);
        assert!(stdout.trim().is_empty(), "quiet should suppress meaningful stdout for {args:?}: {stdout:?}");
    }
}

#[test]
fn e2e_unified_version_and_config_show_contracts() {
    let repo = setup_unified_output_repo();

    let (stdout, stderr, code) = run_rooted(&repo, &["--json", "version"]);
    assert_success(&stdout, &stderr, code, &["--json", "version"]);
    let data = data_object(&stdout);
    for key in ["version", "commit", "build_date", "rustc", "target_triple"] {
        assert!(data.contains_key(key), "version JSON missing {key}: {stdout}");
    }

    let (stdout, stderr, code) = run_rooted(&repo, &["version"]);
    assert_success(&stdout, &stderr, code, &["version"]);
    assert!(stdout.starts_with("mf "), "version text should start with binary name: {stdout}");
    assert!(stdout.contains("built"), "version text missing build metadata: {stdout}");
    assert!(stdout.contains("rustc"), "version text missing rustc metadata: {stdout}");

    let (stdout, stderr, code) = run_rooted(&repo, &["--json", "config", "show"]);
    assert_success(&stdout, &stderr, code, &["--json", "config", "show"]);
    assert!(data_object(&stdout).contains_key("schema_version"), "config data should be structured: {stdout}");
}

#[test]
fn e2e_unified_missing_show_identity_hints_point_to_list_commands() {
    let repo = setup_unified_output_repo();
    let cases = [
        (vec!["project", "show", "missing"], "`mf project list`"),
        (vec!["--project", "alpha", "source", "show", "missing"], "`mf source list`"),
        (vec!["--project", "alpha", "asset", "show", "missing"], "`mf asset list`"),
        (vec!["--project", "alpha", "term", "show", "missing"], "`mf term list`"),
    ];

    for (args, hint) in cases {
        let (_stdout, stderr, code) = run_rooted(&repo, &args);
        assert_ne!(code, 0, "missing identity should fail for {args:?}");
        assert!(stderr.contains("Hint:"), "stderr missing Hint section: {stderr}");
        assert!(stderr.contains(hint), "stderr missing expected hint {hint}: {stderr}");
        assert!(!stderr.contains("'mf "), "stderr should use backticked command examples: {stderr}");
    }
}

#[test]
fn e2e_unified_destructive_confirmation_contract() {
    let repo = setup_unified_output_repo();

    let (_stdout, stderr, code) = run_rooted(&repo, &["project", "remove", "alpha"]);
    assert_ne!(code, 0, "remove without --yes should fail in non-TTY");
    assert!(stderr.contains("pass --yes to confirm"), "stderr missing confirmation hint: {stderr}");
    assert!(repo.path().join("alpha").exists(), "project removed despite missing --yes");

    let (stdout, stderr, code) = run_rooted(&repo, &["project", "remove", "missing-project", "--force"]);
    assert_success(&stdout, &stderr, code, &["project", "remove", "missing-project", "--force"]);
    assert!(stdout.contains("removed project: missing-project"), "force no-op should confirm removal: {stdout}");

    let (stdout, stderr, code) = run_rooted(&repo, &["project", "remove", "alpha", "--yes"]);
    assert_success(&stdout, &stderr, code, &["project", "remove", "alpha", "--yes"]);
    assert!(stdout.contains("removed project: alpha"), "stdout should confirm removal: {stdout}");
    assert!(!repo.path().join("alpha").exists(), "project still exists after --yes remove");
}
