use std::fs;

use crate::datasets::Dataset;
use crate::helpers::*;

/// E2E: 在非 Mind Repo 中运行需上下文的命令应报错
#[test]
fn non_repo_commands_fail_with_clear_error() {
    let outside = Dataset::outside();

    let (stdout, stderr, code) = run_in(outside.path(), &["project", "list"]);

    assert_eq!(code, 1, "exit 1 for non-repo error");
    assert!(stderr.contains("not in a mind repo"), "stderr mentions repo: {stderr}");
    assert!(stdout.is_empty(), "stdout should be empty on error");
}

/// E2E: 在非 Mind Repo 中运行无需上下文的命令应成功
#[test]
fn non_repo_innocent_commands_succeed() {
    let outside = Dataset::outside();

    let (stdout, _, code) = run_in(outside.path(), &["--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("mf"));

    let (stdout, _, code) = run_in(outside.path(), &["completion", "zsh"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("compdef"));

    let (_, _, code) = run_in(outside.path(), &["config", "schema"]);
    assert_eq!(code, 64);
}

/// E2E: minds.yaml 存在时在该目录及子目录中均可识别为 Mind Repo
#[test]
fn repo_detection_from_subdirectory() {
    let ds = Dataset::empty().with_subdir("a/b/c");

    let (_, _, code) = run_in(ds.root().join("a/b/c"), &["term", "list"]);
    assert_eq!(code, 64, "should detect repo from subdirectory");
}

/// E2E: 显式 --config 指向 repo 外部的 mf.yaml 时，以该文件父目录作为 repo root
#[test]
fn config_flag_overrides_repo_search() {
    let ds = Dataset::empty().with_config();
    let outside = Dataset::outside();

    let (_, _, code) = run_in(
        outside.path(),
        &["--config", &ds.root().join("mf.yaml").to_string_lossy(), "term", "list"],
    );
    assert_eq!(code, 64, "--config should allow outside dir to find repo");
}

/// E2E: 创建 minds.yaml 后目录从不属于 repo 变为属于 repo
#[test]
fn create_minds_yaml_establishes_repo() {
    let dir = Dataset::outside();

    // 尚无 minds.yaml
    let (_, stderr, code) = run_in(dir.path(), &["source", "list"]);
    assert_eq!(code, 1);
    assert!(stderr.contains("not in a mind repo"));

    // 创建 minds.yaml
    fs::write(dir.path().join("minds.yaml"), "schema_version: '1'\nprojects: []\n").unwrap();

    // 现在应有 repo
    let (_, _, code) = run_in(dir.path(), &["source", "list"]);
    assert_eq!(code, 64, "now in repo -> placeholder");
}

/// E2E: 在 repo 边界测试 — 刚好在 repo 内成功，上一级失败
#[test]
fn repo_boundary_detection() {
    let ds = Dataset::empty();
    let outside = Dataset::outside();

    // 在 repo 内
    let (_, _, code) = run_in(ds.root(), &["project", "list"]);
    assert_eq!(code, 64, "in repo root");

    // 在 repo 上级目录（parent of repo dir）
    let (_, stderr, code) = run_in(outside.path(), &["project", "list"]);
    assert_eq!(code, 1);
    assert!(stderr.contains("not in a mind repo"));
}

/// E2E: 空 minds.yaml 在未读取 manifest 的命令中不报错
#[test]
fn empty_minds_yaml_still_in_repo() {
    let ds = Dataset::empty_manifest();

    // project list 不读取 manifest，只检测 minds.yaml 存在
    let (_, _, code) = run_in(ds.root(), &["project", "list"]);
    assert_eq!(code, 64, "repo detected, placeholder returned");

    // project index 读取 manifest，应报 parse error
    let (_, stderr, code) = run_in(ds.root(), &["project", "index"]);
    assert_eq!(code, 1, "parse error");
    assert!(stderr.contains("parse error"), "stderr: {stderr}");
}

/// E2E: 不兼容的 schema_version 在使用 manifest 的命令中报错
#[test]
fn incompatible_schema_reports_error() {
    let ds = Dataset::incompatible_schema();

    // project list 不读取 manifest，不报错
    let (_, _, code) = run_in(ds.root(), &["project", "list"]);
    assert_eq!(code, 64, "list does not read manifest");

    // project index 读取 manifest，应报错
    let (_, stderr, code) = run_in(ds.root(), &["project", "index"]);
    assert_eq!(code, 1);
    assert!(stderr.contains("incompatible schema"));
}

/// E2E: 不含 minds.yaml 的目录链应向上一级直到找到
#[test]
fn multi_level_upward_search() {
    let ds = Dataset::empty().with_subdir("x/y/z");

    let (_, _, code) = run_in(ds.root().join("x/y/z"), &["term", "list"]);
    assert_eq!(code, 64, "found repo 3 levels up");
}
