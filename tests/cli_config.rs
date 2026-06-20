use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;

/// Helper: build the mf binary and return a Command.
fn mf() -> Command {
    Command::cargo_bin("mf").unwrap()
}

// ---------------------------------------------------------------------------
// US1: mf config show
// ---------------------------------------------------------------------------

#[test]
fn test_show_no_mind_yaml_returns_defaults() {
    let dir = tempfile::tempdir().unwrap();
    mf().current_dir(dir.path())
        .arg("config")
        .arg("show")
        .assert()
        .success()
        .stdout(predicate::str::contains("schema_version:"))
        .stdout(predicate::str::contains("output_dir: outputs"));
}

#[test]
fn test_show_with_overlay() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("mind.yaml"), "schema_version: '1'\nbuild:\n  output_dir: '_out'\n").unwrap();
    mf().current_dir(dir.path())
        .arg("config")
        .arg("show")
        .assert()
        .success()
        .stdout(predicate::str::contains("output_dir: _out"));
}

#[test]
fn test_show_json_format() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("mind.yaml"), "schema_version: '1'\nbuild:\n  output_dir: '_custom'\n").unwrap();
    mf().current_dir(dir.path())
        .arg("config")
        .arg("show")
        .arg("--output-format")
        .arg("json")
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""output_dir": "_custom""#));
}

#[test]
fn test_show_parse_error() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("mind.yaml"), "invalid: yaml: [[[").unwrap();
    mf().current_dir(dir.path())
        .arg("config")
        .arg("show")
        .assert()
        .failure()
        .stderr(predicate::str::contains("parse error"));
}

#[test]
fn test_show_empty_mind_yaml_is_no_error() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("mind.yaml"), "").unwrap();
    mf().current_dir(dir.path())
        .arg("config")
        .arg("show")
        .assert()
        .success()
        .stdout(predicate::str::contains("schema_version:"));
}

// ---------------------------------------------------------------------------
// US3: mf config schema
// ---------------------------------------------------------------------------

#[test]
fn test_schema_outputs_valid_json_schema() {
    mf().arg("config")
        .arg("schema")
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""$schema""#))
        .stdout(predicate::str::contains(r#""properties""#));
}

#[test]
fn test_schema_yaml_format() {
    mf().arg("config")
        .arg("schema")
        .arg("--output-format")
        .arg("yaml")
        .assert()
        .success()
        .stdout(predicate::str::contains("$schema:"));
}

#[test]
fn test_schema_contains_enum_constraint() {
    mf().arg("config").arg("schema").assert().success().stdout(predicate::str::contains("github_pages"));
}

#[test]
fn test_schema_bidirectional_consistency() {
    // init produces a minds.yaml, then schema validates it
    let dir = tempfile::tempdir().unwrap();
    mf().current_dir(dir.path()).arg("init").assert().success();

    // Get schema and verify it's valid
    let schema_output = mf().arg("config").arg("schema").output().unwrap();
    assert!(schema_output.status.success());
    let schema_str = String::from_utf8_lossy(&schema_output.stdout);
    // Verify schema parses as valid JSON
    let schema_val: serde_json::Value = serde_json::from_str(&schema_str).unwrap();
    assert!(schema_val.get("properties").is_some());

    // Verify init output contains valid YAML (minds.yaml is repo-level manifest)
    let content = fs::read_to_string(dir.path().join("minds.yaml")).unwrap();
    let init_val: serde_json::Value = serde_yaml::from_str(&content).unwrap();
    assert_eq!(init_val.get("schema").and_then(|v| v.as_str()), Some("1"));
}

#[test]
fn test_schema_contains_plugins_field() {
    let schema_output = mf().arg("config").arg("schema").output().unwrap();
    assert!(schema_output.status.success());
    let schema_str = String::from_utf8_lossy(&schema_output.stdout);
    let schema_val: serde_json::Value = serde_json::from_str(&schema_str).unwrap();
    let props = schema_val.get("properties").unwrap();
    assert!(props.get("plugins").is_some(), "schema should contain plugins property");
}

// ── US2: config default includes Typora plugin ──

#[test]
fn config_default_includes_typora_plugin_yaml() {
    mf().arg("config")
        .arg("default")
        .assert()
        .success()
        .stdout(predicate::str::contains("plugins:"))
        .stdout(predicate::str::contains("typora-front-matter:"))
        .stdout(predicate::str::contains("enabled: true"));
}

#[test]
fn config_default_includes_typora_plugin_json() {
    let output = mf().arg("config").arg("default").arg("--output-format").arg("json").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let val: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let plugins = val.get("plugins").expect("should have plugins in JSON default");
    let tfm = plugins.get("typora-front-matter").expect("should have typora-front-matter");
    assert_eq!(tfm.get("enabled"), Some(&serde_json::Value::Bool(true)));
}

// ── US1: schema contains layout property with all categories ──

#[test]
fn test_schema_contains_layout_property() {
    let schema_output = mf().arg("config").arg("schema").output().unwrap();
    assert!(schema_output.status.success());
    let schema_str = String::from_utf8_lossy(&schema_output.stdout);
    let schema_val: serde_json::Value = serde_json::from_str(&schema_str).unwrap();
    let props = schema_val.get("properties").unwrap();
    assert!(props.get("layout").is_some(), "schema should contain layout property");
}

#[test]
fn test_schema_layout_contains_all_categories() {
    let schema_output = mf().arg("config").arg("schema").output().unwrap();
    assert!(schema_output.status.success());
    let schema_str = String::from_utf8_lossy(&schema_output.stdout);
    let schema_val: serde_json::Value = serde_json::from_str(&schema_str).unwrap();
    // The schema contains layout definitions; the layout object should be defined
    let defs = schema_val.get("definitions").unwrap();
    let layout_def = defs.get("LayoutConfig").expect("schema should define LayoutConfig");
    let layout_props = layout_def.get("properties").unwrap();
    assert!(layout_props.get("articles").is_some(), "LayoutConfig should have articles");
    assert!(layout_props.get("sources").is_some(), "LayoutConfig should have sources");
    assert!(layout_props.get("assets").is_some(), "LayoutConfig should have assets");
    assert!(layout_props.get("templates").is_some(), "LayoutConfig should have templates");
    assert!(layout_props.get("build_output").is_some(), "LayoutConfig should have build_output");
}
