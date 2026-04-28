use crate::datasets::Dataset;
use crate::helpers::*;

// ---------------------------------------------------------------------------
// --format json 全局 flag
// ---------------------------------------------------------------------------

/// E2E: --format json 对所有 placeholder 命令输出结构化 JSON
#[test]
fn json_format_for_placeholder() {
    let ds = Dataset::empty();

    let (stdout, stderr, code) = run_in(ds.root(), &["--format", "json", "term", "list"]);

    assert_eq!(code, 64);
    assert!(stderr.is_empty(), "placeholder output to stdout, not stderr");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["status"], "not_implemented");
    assert_eq!(parsed["command"], "mf term list");
}

/// E2E: 默认 text 格式输出 human-readable 信息
#[test]
fn text_format_for_placeholder() {
    let ds = Dataset::empty();

    let (stdout, stderr, code) = run_in(ds.root(), &["term", "list"]);

    assert_eq!(code, 64);
    assert!(stderr.is_empty(), "text output to stdout: {stderr}");
    assert!(stdout.contains("[not implemented]"), "stdout: {stdout}");
    assert!(stdout.contains("mf term list"), "stdout: {stdout}");
}

// ---------------------------------------------------------------------------
// --verbose / --quiet
// ---------------------------------------------------------------------------

/// E2E: --verbose 可叠加
#[test]
fn verbose_flag_accepted() {
    let ds = Dataset::empty();

    let (_, _, code) = run_in(ds.root(), &["-v", "term", "list"]);
    assert_eq!(code, 64);

    let (_, _, code) = run_in(ds.root(), &["-vv", "term", "list"]);
    assert_eq!(code, 64);

    let (_, _, code) = run_in(ds.root(), &["--verbose", "term", "list"]);
    assert_eq!(code, 64);
}

/// E2E: --quiet 抑制非必要输出
#[test]
fn quiet_flag_accepted() {
    let ds = Dataset::empty();

    let (_, _, code) = run_in(ds.root(), &["--quiet", "term", "list"]);
    assert_eq!(code, 64);
}

// ---------------------------------------------------------------------------
// --config 全局 flag
// ---------------------------------------------------------------------------

/// E2E: --config 指向不存在的文件时退化为目录搜索
#[test]
fn config_not_found_error() {
    let outside = Dataset::outside();

    let (_, stderr, code) =
        run_in(outside.path(), &["--config", "/nonexistent/path/mf.yaml", "term", "list"]);
    assert_eq!(code, 1);
    assert!(
        stderr.contains("not in a mind repo"),
        "--config pointing to nonexistent file should fall back to 'not in a mind repo': {stderr}"
    );
}

/// E2E: --config 指向目录时，以该目录为基准查找 minds.yaml
#[test]
fn config_flag_with_directory() {
    let ds = Dataset::empty();
    let outside = Dataset::outside();

    let (_, _, code) =
        run_in(outside.path(), &["--config", &ds.root().to_string_lossy(), "term", "list"]);
    assert_eq!(code, 64, "--config pointing to repo dir should work");
}

// ---------------------------------------------------------------------------
// --help / --version
// ---------------------------------------------------------------------------

/// E2E: --help 在任何目录工作
#[test]
fn help_works_everywhere() {
    let outside = Dataset::outside();
    let (stdout, _, code) = run_in(outside.path(), &["--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("mf"));
    assert!(stdout.contains("project"));
    assert!(stdout.contains("source"));
}

/// E2E: --version 显示版本号
#[test]
fn version_works_everywhere() {
    let outside = Dataset::outside();
    let (stdout, _, code) = run_in(outside.path(), &["--version"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("mf "));
}

/// E2E: 子命令 --help 输出该子命令的用法
#[test]
fn subcommand_help_works() {
    let outside = Dataset::outside();
    let (stdout, _, code) = run_in(outside.path(), &["project", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("Usage:"));
    assert!(stdout.contains("project"));
}

// ---------------------------------------------------------------------------
// 未知命令 / 缺失参数
// ---------------------------------------------------------------------------

/// E2E: 未知子命令报错
#[test]
fn unknown_command_fails() {
    let outside = Dataset::outside();
    let (_, stderr, code) = run_in(outside.path(), &["sourse", "list"]);
    assert_eq!(code, 2);
    assert!(stderr.contains("unrecognized subcommand") || stderr.contains("error"));
}

/// E2E: 缺失必需参数报错
#[test]
fn missing_required_arg_fails() {
    let outside = Dataset::outside();
    let (_, stderr, code) = run_in(outside.path(), &["source", "add"]);
    assert_eq!(code, 2);
    assert!(stderr.contains("required"));
}
