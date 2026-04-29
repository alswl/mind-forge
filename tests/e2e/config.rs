use std::fs;

use crate::datasets::Dataset;
use crate::helpers::*;

/// E2E: `mf config schema` 在 repo 外输出合法 JSON Schema
#[test]
fn schema_outputs_valid_json() {
    let dir = Dataset::outside();

    let (stdout, _, code) = run_in(dir.path(), &["config", "schema"]);
    assert_eq!(code, 0);
    let schema: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert!(schema.get("properties").is_some(), "should have properties");
    assert!(schema.get("definitions").is_some(), "should have definitions");
}

/// E2E: `mf config schema --format yaml` 输出合法 YAML
#[test]
fn schema_outputs_valid_yaml() {
    let dir = Dataset::outside();

    let (stdout, _, code) = run_in(dir.path(), &["config", "schema", "--output-format", "yaml"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("$schema:"), "YAML output: {stdout}");
    assert!(stdout.contains("properties:"), "YAML output: {stdout}");
}

/// E2E: `mf config show` 在无 mind.yaml 时输出内置默认
#[test]
fn show_defaults_when_no_mind_yaml() {
    let dir = Dataset::outside();

    let (stdout, _, code) = run_in(dir.path(), &["config", "show"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("schema_version:"), "stdout: {stdout}");
    assert!(stdout.contains("output_dir: _build"), "stdout: {stdout}");
}

/// E2E: `mf config show` 在 repo 内子目录向上找到 mind.yaml
#[test]
fn show_finds_mind_yaml_in_parent() {
    let ds = Dataset::empty().with_subdir("a/b/c");
    // 在项目目录创建 mind.yaml（与 minds.yaml 同级）
    fs::write(ds.root().join("mind.yaml"), "schema_version: '1'\nbuild:\n  output_dir: _custom\n")
        .unwrap();

    let (stdout, _, code) = run_in(ds.root().join("a/b/c"), &["config", "show"]);
    assert_eq!(code, 0, "show should succeed from subdirectory");
    assert!(stdout.contains("output_dir: _custom"), "should find overlay: {stdout}");
}

/// E2E: `mf config show` 在 repo 根目录也能找到 mind.yaml
#[test]
fn show_finds_mind_yaml_at_repo_root() {
    let ds = Dataset::empty();
    fs::write(ds.root().join("mind.yaml"), "schema_version: '1'\nbuild:\n  output_dir: _custom\n")
        .unwrap();

    let (stdout, _, code) = run_in(ds.root(), &["config", "show"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("output_dir: _custom"));
}

/// E2E: `mf config show` 空 mind.yaml 无错误
#[test]
fn show_empty_mind_yaml_no_error() {
    let ds = Dataset::empty();
    fs::write(ds.root().join("mind.yaml"), "").unwrap();

    let (stdout, _, code) = run_in(ds.root(), &["config", "show"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("schema_version:"));
}

/// E2E: `mf config show` 非法 YAML → parse error
#[test]
fn show_invalid_yaml_returns_parse_error() {
    let ds = Dataset::empty();
    fs::write(ds.root().join("mind.yaml"), "invalid: yaml: [[[").unwrap();

    let (_, stderr, code) = run_in(ds.root(), &["config", "show"]);
    assert_eq!(code, 1);
    assert!(stderr.contains("parse error"), "stderr: {stderr}");
}

/// E2E: `mf config show --format json` 输出合法 JSON
#[test]
fn show_json_output() {
    let ds = Dataset::empty();
    fs::write(ds.root().join("mind.yaml"), "schema_version: '1'\nbuild:\n  output_dir: _custom\n")
        .unwrap();

    let (stdout, _, code) = run_in(ds.root(), &["config", "show", "--output-format", "json"]);
    assert_eq!(code, 0);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["build"]["output_dir"], "_custom");
}

/// E2E: `mf config init` 创建 mind.yaml
#[test]
fn init_creates_mind_yaml() {
    let ds = Dataset::empty();

    let (_, _, code) = run_in(ds.root(), &["config", "init"]);
    assert_eq!(code, 0);
    let path = ds.root().join("mind.yaml");
    assert!(path.exists(), "mind.yaml should exist");
    let content = fs::read_to_string(&path).unwrap();
    assert!(content.contains("schema_version:"));
    assert!(content.contains("name:"));
}

/// E2E: `mf config init` 拒绝重复
#[test]
fn init_refuses_duplicate() {
    let ds = Dataset::empty();
    fs::write(ds.root().join("mind.yaml"), "schema_version: '1'\n").unwrap();

    let (_, stderr, code) = run_in(ds.root(), &["config", "init"]);
    assert_eq!(code, 1);
    assert!(stderr.contains("refusing to overwrite"), "stderr: {stderr}");
}

/// E2E: `mf config init --force` 覆盖
#[test]
fn init_force_overwrites() {
    let ds = Dataset::empty();
    fs::write(ds.root().join("mind.yaml"), "schema_version: '1'\n").unwrap();

    let (_, _, code) = run_in(ds.root(), &["config", "init", "--force"]);
    assert_eq!(code, 0);
    assert!(ds.root().join("mind.yaml").exists());
}

/// E2E: `mf config init --target user` → exit 64 + not implemented
#[test]
fn init_target_user_not_implemented() {
    let ds = Dataset::empty();

    let (_, stderr, code) = run_in(ds.root(), &["config", "init", "--target", "user"]);
    assert_eq!(code, 64);
    assert!(stderr.contains("not yet implemented"), "stderr: {stderr}");
}

/// E2E: 完整往返 — init → show → schema 校验
#[test]
fn full_roundtrip_init_show_schema() {
    let ds = Dataset::empty();

    // init
    let (_, _, code) = run_in(ds.root(), &["config", "init"]);
    assert_eq!(code, 0, "init");

    // show 包含 init 写入的字段
    let (stdout, _, code) = run_in(ds.root(), &["config", "show"]);
    assert_eq!(code, 0, "show after init");
    assert!(stdout.contains("schema_version:"), "schema_version in show: {stdout}");
    assert!(stdout.contains("name:"), "project name in show: {stdout}");

    // schema 输出合法
    let (schema_stdout, _, code) = run_in(ds.root(), &["config", "schema"]);
    assert_eq!(code, 0, "schema after init");
    let schema: serde_json::Value =
        serde_json::from_str(&schema_stdout).expect("schema is valid JSON");
    assert!(schema.get("properties").is_some(), "schema has properties");
}
