use std::fs;

use crate::datasets::{self, Dataset};
use crate::helpers::*;

// ---------------------------------------------------------------------------
// mf project index — E2E 场景
// ---------------------------------------------------------------------------

/// E2E: 创建一个项目后 index，minds.yaml 应包含该项目的 projects 条目
#[test]
fn index_discovers_new_project() {
    let ds = Dataset::empty().with_project("my-doc");

    let (_, _, code) = run_in(ds.root(), &["project", "index"]);
    assert_eq!(code, 64, "placeholder exit");

    let content = ds.read_manifest();
    assert!(content.contains("my-doc"), "manifest should contain new project: {content}");
    assert!(content.contains("projects"), "manifest should have projects key");
}

/// E2E: 删除项目目录后 index，minds.yaml 应移除该条目
#[test]
fn index_removes_deleted_project() {
    let ds = Dataset::empty().with_project("to-go");

    // 先注册项目
    run_in(ds.root(), &["project", "index"]);
    assert!(ds.read_manifest().contains("to-go"));

    // 删除目录并重新 index
    fs::remove_dir_all(ds.root().join("to-go")).unwrap();
    run_in(ds.root(), &["project", "index"]);

    let content = ds.read_manifest();
    assert!(!content.contains("to-go"), "removed project should be gone: {content}");
}

/// E2E: 多个项目应全部注册
#[test]
fn index_discovers_multiple_projects() {
    let ds = datasets::repo_with_three_projects();

    run_in(ds.root(), &["project", "index"]);
    let content = ds.read_manifest();

    assert!(content.contains("alpha"), "{content}");
    assert!(content.contains("beta"), "{content}");
    assert!(content.contains("gamma"), "{content}");
}

/// E2E: 不含 mind.yaml 的目录应被忽略
#[test]
fn index_ignores_non_project_dirs() {
    let ds = datasets::repo_with_mixed_content();

    run_in(ds.root(), &["project", "index"]);
    let content = ds.read_manifest();

    assert!(content.contains("real-project"));
    assert!(!content.contains("just-a-folder"), "non-project should be ignored: {content}");
    assert!(!content.contains("another-folder"), "non-project should be ignored: {content}");
}

/// E2E: dry-run 模式不修改文件
#[test]
fn index_dry_run_does_not_modify() {
    let ds = Dataset::empty();
    let before = ds.read_manifest();

    fs::create_dir_all(ds.root().join("new-project")).unwrap();
    fs::write(ds.root().join("new-project/mind.yaml"), "schema_version: '1'\n").unwrap();
    run_in(ds.root(), &["project", "index", "--dry-run"]);

    let after = ds.read_manifest();
    assert_eq!(before, after, "dry-run should not modify minds.yaml");
}

/// E2E: minds.yaml 不存在时自动创建
#[test]
fn index_creates_minds_yaml_when_absent() {
    let dir = Dataset::outside();
    assert!(!dir.path().join("minds.yaml").exists());

    fs::create_dir_all(dir.path().join("new-project")).unwrap();
    fs::write(dir.path().join("new-project/mind.yaml"), "schema_version: '1'\n").unwrap();

    run_in(dir.path(), &["project", "index"]);

    assert!(dir.path().join("minds.yaml").exists(), "minds.yaml should be created");
    let content = std::fs::read_to_string(dir.path().join("minds.yaml")).unwrap();
    assert!(content.contains("new-project"));
}

/// E2E: 文件为普通文件而非目录时的行为
#[test]
fn index_skips_files_not_dirs() {
    let ds = Dataset::empty();
    fs::write(ds.root().join("readme.md"), "hello").unwrap();

    run_in(ds.root(), &["project", "index"]);
    let content = ds.read_manifest();
    assert!(!content.contains("readme"));
}

/// E2E: schema_version 不兼容时报错
#[test]
fn index_rejects_incompatible_schema() {
    let ds = Dataset::incompatible_schema().with_project("p1");

    let (_, stderr, code) = run_in(ds.root(), &["project", "index"]);
    assert_eq!(code, 1);
    assert!(stderr.contains("incompatible schema"));
}

/// E2E: --format json 时 project index 输出为 JSON
#[test]
fn index_json_output_format() {
    let ds = Dataset::empty().with_project("json-project");

    let (stdout, _, code) = run_in(ds.root(), &["--format", "json", "project", "index"]);
    assert_eq!(code, 64);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["status"], "not_implemented");
    assert_eq!(parsed["command"], "mf project index");
}

/// E2E: 连续两次 index 是幂等的
#[test]
fn index_is_idempotent() {
    let ds = Dataset::empty().with_project("stable");

    run_in(ds.root(), &["project", "index"]);
    let after_first = ds.read_manifest();

    run_in(ds.root(), &["project", "index"]);
    let after_second = ds.read_manifest();

    assert_eq!(after_first, after_second, "index should be idempotent");
}
