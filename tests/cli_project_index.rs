use assert_cmd::Command;
use std::fs;

mod common;

#[test]
fn test_index_adds_new_project() {
    let dir = common::setup_repo();
    common::create_project(&dir, "new-project");
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.current_dir(dir.path()).arg("project").arg("index");
    cmd.assert().code(0);
    let content = fs::read_to_string(dir.path().join("minds.yaml")).unwrap();
    assert!(content.contains("new-project"));
}

#[test]
fn test_index_removes_deleted_project() {
    let dir = common::setup_repo();
    common::create_project(&dir, "to-delete");
    let mut cmd1 = Command::cargo_bin("mf").unwrap();
    cmd1.current_dir(dir.path()).arg("project").arg("index").assert().code(0);
    fs::remove_dir_all(dir.path().join("to-delete")).unwrap();
    let mut cmd2 = Command::cargo_bin("mf").unwrap();
    cmd2.current_dir(dir.path()).arg("project").arg("index").assert().code(0);
    let content = fs::read_to_string(dir.path().join("minds.yaml")).unwrap();
    assert!(!content.contains("to-delete"));
}

#[test]
fn test_index_creates_minds_yaml_when_absent() {
    let dir = tempfile::TempDir::new().unwrap();
    common::create_project(&dir, "some-project");
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.current_dir(dir.path()).arg("project").arg("index");
    cmd.assert().code(0);
    assert!(dir.path().join("minds.yaml").exists());
}

#[test]
fn test_index_dry_run_does_not_modify() {
    let dir = common::setup_repo();
    common::create_project(&dir, "new-project");
    let before = fs::read_to_string(dir.path().join("minds.yaml")).unwrap();
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.current_dir(dir.path()).arg("project").arg("index").arg("--dry-run");
    cmd.assert().code(0);
    let after = fs::read_to_string(dir.path().join("minds.yaml")).unwrap();
    assert_eq!(before, after);
}

#[test]
fn test_index_works_in_non_repo() {
    let dir = tempfile::TempDir::new().unwrap();
    common::create_project(&dir, "some-project");
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.current_dir(dir.path()).arg("project").arg("index");
    cmd.assert().code(0);
    assert!(dir.path().join("minds.yaml").exists());
}
