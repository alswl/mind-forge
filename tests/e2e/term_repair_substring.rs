use assert_cmd::Command;
use std::fs;

#[path = "../common/mod.rs"]
mod common;

fn mf(repo: &common::TempDir) -> Command {
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.args(["--root", repo.path().to_str().unwrap()]);
    cmd
}

#[test]
fn supported_substring_project_and_global_workflow() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let project = repo.path().join("alpha");
    fs::create_dir_all(project.join("docs")).unwrap();

    common::write_index(
        &repo,
        "alpha",
        r#"schema_version: '1'
terms:
  - term: LegacyProject
    corrections:
      - original: legacy
        correct: LegacyProject
        match: substring
"#,
    );
    fs::write(
        repo.path().join("minds-terms.yaml"),
        r#"schema_version: '1'
terms:
  - term: LegacyGlobal
    corrections:
      - original: legacyg
        correct: LegacyGlobal
        match: substring
"#,
    )
    .unwrap();
    fs::write(project.join("docs/project.md"), "legacy appears here\n").unwrap();
    fs::create_dir_all(repo.path().join("docs")).unwrap();
    fs::write(repo.path().join("docs").join("global.md"), "legacyg appears here\n").unwrap();

    let project_scan = mf(&repo).args(["term", "lint", "--project", "alpha"]).output().unwrap();
    assert_eq!(project_scan.status.code(), Some(1), "substring should reach scanning");

    let global_scan = mf(&repo).args(["term", "lint"]).output().unwrap();
    assert_ne!(global_scan.status.code(), Some(2), "substring must not poison global loading");

    let show_project = mf(&repo)
        .args(["term", "correction", "show", "LegacyProject", "legacy", "--project", "alpha"])
        .output()
        .unwrap();
    assert!(show_project.status.success());
    assert!(String::from_utf8(show_project.stdout).unwrap().contains("match: substring"));

    let list_global = mf(&repo).args(["term", "correction", "list", "LegacyGlobal"]).output().unwrap();
    assert!(list_global.status.success());
    assert!(String::from_utf8(list_global.stdout).unwrap().contains("match=substring"));

    let repair_project = mf(&repo)
        .args(["term", "correction", "update", "LegacyProject", "legacy", "--match", "word", "--project", "alpha"])
        .output()
        .unwrap();
    assert!(repair_project.status.success(), "stderr: {}", String::from_utf8_lossy(&repair_project.stderr));

    let repair_global = mf(&repo).args(["term", "correction", "remove", "LegacyGlobal", "legacyg"]).output().unwrap();
    assert!(repair_global.status.success(), "stderr: {}", String::from_utf8_lossy(&repair_global.stderr));

    let project_lint = mf(&repo).args(["term", "lint", "--project", "alpha"]).output().unwrap();
    assert_eq!(project_lint.status.code(), Some(1), "updated project should continue scanning");
    let global_lint = mf(&repo).args(["term", "lint"]).output().unwrap();
    assert_eq!(global_lint.status.code(), Some(0), "removed global correction should leave repo clean");
}
