use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;

mod common;

/// --root 指向非 repo 目录报错（FR-501）
#[test]
fn root_flag_rejects_non_repo() {
    let outside = tempfile::TempDir::new().unwrap();
    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(&outside)
        .args(["--root", &outside.path().to_string_lossy(), "project", "list"])
        .assert()
        .code(predicate::eq(1))
        .stderr(predicate::str::contains("not in a mind repo"));
}

/// --root 从 repo 外操作成功（FR-504）
#[test]
fn root_flag_overrides_cwd() {
    let dir = common::setup_repo();
    let outside = tempfile::TempDir::new().unwrap();
    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(outside.path())
        .args(["--root", &dir.path().to_string_lossy(), "project", "list"])
        .assert()
        .code(predicate::eq(0));
}

#[test]
fn verbose_and_quiet_conflict() {
    let dir = common::setup_repo();
    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(dir.path())
        .args(["--verbose", "--quiet", "source", "list"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("'--verbose' cannot be used with '--quiet'"));
}

#[test]
fn json_placeholder_uses_json_shape() {
    let dir = common::setup_repo();
    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(dir.path())
        .args(["--format", "json", "source", "list"])
        .assert()
        .code(64)
        .stdout(predicate::str::contains("\"status\": \"not_implemented\""))
        .stdout(predicate::str::contains("\"command\": \"mf source list\""));
}

#[test]
fn explicit_config_path_is_accepted() {
    let dir = common::setup_repo();
    let config_path = dir.path().join("mf.yaml");
    fs::write(&config_path, "schema_version: '1'\n").unwrap();
    Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(dir.path())
        .args(["--config", &config_path.to_string_lossy(), "term", "list"])
        .assert()
        .code(64)
        .stdout(predicate::str::contains("[not implemented] mf term list"));
}
