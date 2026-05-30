//! Integration tests for `mf publish target list` (formerly `mf publisher list`).
//!
//! Covers publisher discovery and listing output.

use assert_cmd::Command;

mod common;

fn run_in_repo(repo: &common::TempDir, args: &[&str]) -> std::process::Output {
    Command::cargo_bin("mf").expect("binary exists").current_dir(repo.path()).args(args).output().expect("command runs")
}

// ---------------------------------------------------------------------------
// T014: Missing directory returns empty publishers and diagnostics
// ---------------------------------------------------------------------------

#[test]
fn missing_publisher_dir_returns_empty_json() {
    let repo = common::setup_repo();

    let out = run_in_repo(&repo, &["--format", "json", "publish", "target", "list"]);
    assert_eq!(out.status.code(), Some(0));

    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["status"], "ok");
    let data = &v["data"];
    assert_eq!(data["publish_targets"], serde_json::json!([]));
    assert_eq!(data["diagnostics"], serde_json::json!([]));
}

// ---------------------------------------------------------------------------
// T015: foo.yaml without explicit name discovers publisher "foo"
// ---------------------------------------------------------------------------

#[test]
fn file_stem_is_default_name_text() {
    let repo = common::setup_repo();
    common::write_publisher_yaml(&repo, "foo", "type: local\nenabled: true\nconfig:\n  path: ./output\n");

    let out = run_in_repo(&repo, &["publish", "target", "list"]);
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("foo"), "stdout should contain publisher name 'foo': {stdout}");
}

#[test]
fn file_stem_is_default_name_json() {
    let repo = common::setup_repo();
    common::write_publisher_yaml(&repo, "foo", "type: local\nenabled: true\nconfig:\n  path: ./output\n");

    let out = run_in_repo(&repo, &["--format", "json", "publish", "target", "list"]);
    assert_eq!(out.status.code(), Some(0));

    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let publishers = &v["data"]["publish_targets"];
    assert_eq!(publishers.as_array().unwrap().len(), 1);
    assert_eq!(publishers[0]["name"], "foo");
    assert_eq!(publishers[0]["target_type"], "local");
    assert_eq!(publishers[0]["status"], "available");
    assert_eq!(publishers[0]["source_path"], ".mind-forge/publisher/foo.yaml");
}

// ---------------------------------------------------------------------------
// T016: Multiple publishers sorted by name
// ---------------------------------------------------------------------------

#[test]
fn sorted_publishers_json() {
    let repo = common::setup_repo();
    common::write_publishers(
        &repo,
        &[
            ("zeta", "type: local\nenabled: true\nconfig:\n  path: ./z\n"),
            ("alpha", "type: local\nenabled: true\nconfig:\n  path: ./a\n"),
            ("beta", "type: local\nenabled: true\nconfig:\n  path: ./b\n"),
        ],
    );

    let out = run_in_repo(&repo, &["--format", "json", "publish", "target", "list"]);
    assert_eq!(out.status.code(), Some(0));

    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let publishers = &v["data"]["publish_targets"];
    assert_eq!(publishers.as_array().unwrap().len(), 3);
    assert_eq!(publishers[0]["name"], "alpha");
    assert_eq!(publishers[1]["name"], "beta");
    assert_eq!(publishers[2]["name"], "zeta");
}

// ---------------------------------------------------------------------------
// US3 — Diagnostic invalid definitions (T036–T040)
// ---------------------------------------------------------------------------

#[test]
fn malformed_yaml_returns_diagnostic_valid_still_listed() {
    let repo = common::setup_repo();
    common::write_publishers(
        &repo,
        &[("good", "type: local\nenabled: true\nconfig:\n  path: ./out\n"), ("bad", "this is not valid yaml: [[[,,")],
    );

    let out = run_in_repo(&repo, &["--format", "json", "publish", "target", "list"]);
    assert_eq!(out.status.code(), Some(0));

    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let data = &v["data"];

    // Valid publisher should still be listed
    let publishers = data["publish_targets"].as_array().unwrap();
    assert_eq!(publishers.len(), 1);
    assert_eq!(publishers[0]["name"], "good");

    // Malformed file should produce diagnostic
    let diagnostics = data["diagnostics"].as_array().unwrap();
    assert!(!diagnostics.is_empty(), "malformed YAML should produce a diagnostic");
    assert_eq!(diagnostics[0]["kind"], "malformed_yaml");
    assert!(diagnostics[0]["path"].as_str().unwrap_or("").contains("bad.yaml"));
    assert!(!diagnostics[0]["message"].as_str().unwrap_or("").is_empty());
}

#[test]
fn missing_type_returns_missing_required_field() {
    let repo = common::setup_repo();
    // Missing both `type` and `config.path`
    common::write_publisher_yaml(&repo, "notype", "enabled: true\n");

    let out = run_in_repo(&repo, &["--format", "json", "publish", "target", "list"]);
    assert_eq!(out.status.code(), Some(0));

    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let diagnostics = &v["data"]["diagnostics"];
    assert_eq!(diagnostics.as_array().unwrap().len(), 1);
    assert_eq!(diagnostics[0]["kind"], "missing_required_field");
    assert!(diagnostics[0]["message"].as_str().unwrap_or("").contains("type"));
}

#[test]
fn local_missing_config_path_returns_diagnostic() {
    let repo = common::setup_repo();
    common::write_publisher_yaml(&repo, "nopath", "type: local\nenabled: true\n");

    let out = run_in_repo(&repo, &["--format", "json", "publish", "target", "list"]);
    assert_eq!(out.status.code(), Some(0));

    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let diagnostics = &v["data"]["diagnostics"];
    assert_eq!(diagnostics.as_array().unwrap().len(), 1);
    assert_eq!(diagnostics[0]["kind"], "missing_required_field");
    assert!(diagnostics[0]["message"].as_str().unwrap_or("").contains("config.path"));
}

#[test]
fn invalid_name_returns_invalid_name_diagnostic() {
    let repo = common::setup_repo();
    common::write_publisher_yaml(
        &repo,
        "has_underscore",
        "name: has_underscore\ntype: local\nenabled: true\nconfig:\n  path: ./out\n",
    );

    let out = run_in_repo(&repo, &["--format", "json", "publish", "target", "list"]);
    assert_eq!(out.status.code(), Some(0));

    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let diagnostics = &v["data"]["diagnostics"];
    let invalid_names: Vec<_> =
        diagnostics.as_array().unwrap().iter().filter(|d| d["kind"] == "invalid_name").collect();
    assert!(!invalid_names.is_empty(), "should have invalid_name diagnostic");
}

#[test]
fn reserved_name_returns_reserved_name_diagnostic() {
    let repo = common::setup_repo();
    common::write_publisher_yaml(&repo, "default", "type: local\nenabled: true\nconfig:\n  path: ./out\n");

    let out = run_in_repo(&repo, &["--format", "json", "publish", "target", "list"]);
    assert_eq!(out.status.code(), Some(0));

    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let diagnostics = &v["data"]["diagnostics"];
    let reserved: Vec<_> = diagnostics.as_array().unwrap().iter().filter(|d| d["kind"] == "reserved_name").collect();
    assert!(!reserved.is_empty(), "should have reserved_name diagnostic");
    assert!(
        reserved[0]["message"].as_str().unwrap_or("").contains("default"),
        "message should mention the reserved name"
    );
}

#[test]
fn duplicate_name_returns_duplicate_name_diagnostic() {
    let repo = common::setup_repo();
    common::write_publishers(
        &repo,
        &[
            ("blog-a", "name: blog\ntype: local\nenabled: true\nconfig:\n  path: ./a\n"),
            ("blog-b", "name: blog\ntype: local\nenabled: true\nconfig:\n  path: ./b\n"),
        ],
    );

    let out = run_in_repo(&repo, &["--format", "json", "publish", "target", "list"]);
    assert_eq!(out.status.code(), Some(0));

    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let diagnostics = &v["data"]["diagnostics"];
    let dup: Vec<_> = diagnostics.as_array().unwrap().iter().filter(|d| d["kind"] == "duplicate_name").collect();
    assert!(!dup.is_empty(), "should have duplicate_name diagnostic");
    // At most one publisher named "blog" should be usable
    let publishers = &v["data"]["publish_targets"];
    let blog_publishers: Vec<_> = publishers.as_array().unwrap().iter().filter(|p| p["name"] == "blog").collect();
    assert_eq!(blog_publishers.len(), 1, "only one blog publisher should be listed");
}

#[test]
fn disabled_publisher_shows_disabled_status() {
    let repo = common::setup_repo();
    common::write_publisher_yaml(&repo, "offline", "type: local\nenabled: false\nconfig:\n  path: ./out\n");

    let out = run_in_repo(&repo, &["--format", "json", "publish", "target", "list"]);
    assert_eq!(out.status.code(), Some(0));

    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let publishers = &v["data"]["publish_targets"];
    assert_eq!(publishers.as_array().unwrap().len(), 1);
    assert_eq!(publishers[0]["name"], "offline");
    assert_eq!(publishers[0]["status"], "disabled");
    assert_eq!(publishers[0]["enabled"], false);

    // A disabled publisher should not produce diagnostics — it's a valid state
    let diagnostics = &v["data"]["diagnostics"];
    assert!(diagnostics.as_array().unwrap().is_empty());
}

#[test]
fn secret_field_returns_secret_field_diagnostic() {
    let repo = common::setup_repo();
    common::write_publisher_yaml(
        &repo,
        "leaky",
        "type: local\nenabled: true\nconfig:\n  path: ./out\n  token: my-secret-token\n",
    );

    let out = run_in_repo(&repo, &["--format", "json", "publish", "target", "list"]);
    assert_eq!(out.status.code(), Some(0));

    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let diagnostics = &v["data"]["diagnostics"];
    let secret: Vec<_> = diagnostics.as_array().unwrap().iter().filter(|d| d["kind"] == "secret_field").collect();
    assert!(!secret.is_empty(), "should have secret_field diagnostic");
    assert!(
        secret[0]["message"].as_str().unwrap_or("").contains("token"),
        "message should mention the secret field name"
    );
    assert!(secret[0]["hint"].as_str().unwrap_or("").contains("environment"), "hint should suggest moving to env vars");
}

#[test]
fn secret_field_blocks_publisher_listing() {
    let repo = common::setup_repo();
    common::write_publisher_yaml(
        &repo,
        "leaky",
        "type: local\nenabled: true\nconfig:\n  path: ./out\n  api_key: sk-1234\n",
    );

    let out = run_in_repo(&repo, &["--format", "json", "publish", "target", "list"]);
    assert_eq!(out.status.code(), Some(0));

    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    // Secret-bearing publisher should not be in the publishers list
    let publishers = &v["data"]["publish_targets"];
    assert!(
        !publishers.as_array().unwrap().iter().any(|p| p["name"] == "leaky"),
        "publisher with secret field should not be listed as available"
    );
}

// ---------------------------------------------------------------------------
// T053: Performance regression — 10 publisher files
// ---------------------------------------------------------------------------

#[test]
fn discover_10_publishers_performance() {
    let repo = common::setup_repo();
    let publisher_names: Vec<_> = (0..10).map(|i| format!("pub-{i:02}")).collect();
    for name in &publisher_names {
        common::write_publisher_yaml(
            &repo,
            name,
            &format!("name: {name}\ntype: local\nenabled: true\nconfig:\n  path: ./output/{name}\n"),
        );
    }

    let start = std::time::Instant::now();
    let out = run_in_repo(&repo, &["--format", "json", "publish", "target", "list"]);
    let elapsed = start.elapsed();

    assert_eq!(out.status.code(), Some(0));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let publishers = v["data"]["publish_targets"].as_array().unwrap();
    assert_eq!(publishers.len(), 10);

    // Discovery should complete well under 100ms for 10 files
    assert!(elapsed.as_millis() < 200, "discovery of 10 publishers took {}ms (expected <200ms)", elapsed.as_millis());
}
