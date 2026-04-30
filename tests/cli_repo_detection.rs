use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;

mod common;

#[test]
fn test_non_repo_returns_not_in_mind_repo() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.current_dir(dir.path()).arg("project").arg("list");
    cmd.assert().code(predicate::eq(1)).stderr(predicate::str::contains("not in a mind repo"));
}

#[test]
fn test_in_repo_succeeds() {
    let dir = common::setup_repo();
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.current_dir(dir.path()).arg("project").arg("list");
    cmd.assert().code(predicate::eq(0));
}

#[test]
fn test_non_repo_help_succeeds() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.current_dir(dir.path()).arg("--help");
    cmd.assert().code(predicate::eq(0));
}

#[test]
fn test_non_repo_config_schema_succeeds() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.current_dir(dir.path()).arg("config").arg("schema");
    cmd.assert().code(predicate::eq(0));
}

#[test]
fn test_non_repo_completion_succeeds() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.current_dir(dir.path()).arg("completion").arg("zsh");
    cmd.assert().code(predicate::eq(0));
}

#[test]
fn test_repo_subdirectory_detection() {
    let dir = common::setup_repo();
    let sub = dir.path().join("deep").join("nested").join("path");
    fs::create_dir_all(&sub).unwrap();
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.current_dir(&sub).arg("project").arg("list");
    cmd.assert().code(predicate::eq(0));
}

#[test]
fn test_config_flag_overrides_search() {
    let dir = tempfile::TempDir::new().unwrap();
    let repo_dir = common::setup_repo();
    fs::write(repo_dir.path().join("mf.yaml"), "schema_version: '1'\n").unwrap();
    let non_repo_sub = dir.path().join("sub");
    fs::create_dir_all(&non_repo_sub).unwrap();
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.current_dir(&non_repo_sub)
        .arg("--config")
        .arg(repo_dir.path().join("mf.yaml"))
        .arg("project")
        .arg("list");
    cmd.assert().code(predicate::eq(0));
}
