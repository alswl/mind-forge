use crate::datasets::Dataset;
use crate::helpers::*;

// ---------------------------------------------------------------------------
// JSON error envelope schema
// ---------------------------------------------------------------------------

/// E2E: JSON 格式的 error envelope 包含 status/command/error.kind/error.message/error.hint
#[test]
fn json_error_envelope_structure() {
    let outside = Dataset::outside();

    let (_stdout, stderr, code) = run_in(outside.path(), &["--format", "json", "project", "list"]);

    assert_eq!(code, 1);

    let parsed: serde_json::Value =
        serde_json::from_str(&stderr).expect("valid JSON error envelope");
    assert_eq!(parsed["status"], "error");
    assert_eq!(parsed["command"], "mf");
    assert_eq!(parsed["error"]["kind"], "not-in-mind-repo");
    assert!(parsed["error"]["message"].as_str().unwrap_or("").contains("not in a mind repo"));
    assert!(parsed["error"]["hint"].is_string());
}

/// E2E: JSON 格式的 usage 错误
#[test]
fn json_usage_error_envelope() {
    let outside = Dataset::outside();

    let (_stdout, stderr, code) =
        run_in(outside.path(), &["--format", "json", "--verbose", "--quiet", "source", "list"]);

    assert_eq!(code, 2);

    let parsed: serde_json::Value = serde_json::from_str(&stderr).expect("valid JSON");
    assert_eq!(parsed["status"], "error");
    assert_eq!(parsed["error"]["kind"], "usage");
}

/// E2E: JSON 格式的 incompatible-schema 错误
#[test]
fn json_incompatible_schema_envelope() {
    let ds = Dataset::incompatible_schema();

    let (_stdout, stderr, code) = run_in(ds.root(), &["--format", "json", "project", "index"]);

    assert_eq!(code, 1);

    let parsed: serde_json::Value = serde_json::from_str(&stderr).expect("valid JSON");
    assert_eq!(parsed["error"]["kind"], "incompatible-schema");
}

/// E2E: JSON 格式的 parse-error 错误
#[test]
fn json_parse_error_envelope() {
    let ds = Dataset::not_yaml();

    let (_stdout, stderr, code) = run_in(ds.root(), &["--format", "json", "project", "index"]);

    assert_eq!(code, 1);

    let parsed: serde_json::Value = serde_json::from_str(&stderr).expect("valid JSON");
    assert_eq!(parsed["error"]["kind"], "parse-error");
}

/// E2E: CLI parse 错误（如未知 flag）在 JSON 模式下仍输出 JSON error
#[test]
fn json_cli_parse_error_envelope() {
    let ds = Dataset::empty();

    let (_stdout, stderr, code) =
        run_in(ds.root(), &["--format", "json", "term", "list", "--bogus-flag"]);

    assert_eq!(code, 2);

    let parsed: serde_json::Value = serde_json::from_str(&stderr).expect("valid JSON");
    assert_eq!(parsed["error"]["kind"], "usage");
}

/// E2E: 文本格式中 usage 错误应包含 hint
#[test]
fn text_usage_has_hint() {
    let ds = Dataset::empty();

    let (_, stderr, code) = run_in(ds.root(), &["build", ""]);

    assert_eq!(code, 2);
    assert!(stderr.contains("mf article list"), "hint should mention article list: {stderr}");
}

/// E2E: 文本格式中 not-in-mind-repo 应包含 hint
#[test]
fn text_not_in_mind_repo_has_hint() {
    let outside = Dataset::outside();

    let (_, stderr, code) = run_in(outside.path(), &["project", "list"]);

    assert_eq!(code, 1);
    assert!(stderr.contains("not in a mind repo"), "error message: {stderr}");
    assert!(stderr.contains("Hint:"), "should have hint: {stderr}");
}

/// E2E: 工具错误（如不兼容的 schema）应包含 hint
#[test]
fn text_incompatible_schema_has_hint() {
    let ds = Dataset::incompatible_schema();

    let (_, stderr, code) = run_in(ds.root(), &["project", "index"]);

    assert_eq!(code, 1);
    assert!(stderr.contains("incompatible schema"));
    assert!(stderr.contains("Hint:"), "should have upgrade hint: {stderr}");
}

/// E2E: JSON error envelope 中的 hint 字段类型应为 string 或 null
#[test]
fn json_error_hint_is_string_or_null() {
    let outside = Dataset::outside();

    let (_, stderr, _) = run_in(outside.path(), &["--format", "json", "project", "list"]);
    let parsed: serde_json::Value = serde_json::from_str(&stderr).expect("valid JSON");
    let hint = &parsed["error"]["hint"];
    assert!(hint.is_string() || hint.is_null(), "hint should be string or null, got: {hint}");
}
