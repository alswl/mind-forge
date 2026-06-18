use assert_cmd::Command;

mod common;

#[test]
fn update_project_description() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "project",
            "update",
            "alpha",
            "--description",
            "Research notes",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let mind_yaml = std::fs::read_to_string(repo.path().join("alpha/mind.yaml")).unwrap();
    assert!(mind_yaml.contains("description: Research notes"), "mind.yaml: {mind_yaml}");
}

#[test]
fn update_project_clear_description() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    common::write_mind_yaml(&repo, "alpha", "schema_version: '1'\nproject:\n  name: alpha\n  description: Old text\n");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "project", "update", "alpha", "--clear-description"])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let mind_yaml = std::fs::read_to_string(repo.path().join("alpha/mind.yaml")).unwrap();
    assert!(!mind_yaml.contains("description:"), "mind.yaml: {mind_yaml}");
}

#[test]
fn update_project_dry_run_does_not_write() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let before = std::fs::read_to_string(repo.path().join("alpha/mind.yaml")).unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "project",
            "update",
            "alpha",
            "--description",
            "Preview only",
            "--dry-run",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let after = std::fs::read_to_string(repo.path().join("alpha/mind.yaml")).unwrap();
    assert_eq!(after, before);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("[dry-run] would update project: alpha"));
}

#[test]
fn update_project_json_envelope() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "--format",
            "json",
            "project",
            "update",
            "alpha",
            "--description",
            "Research notes",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    assert_eq!(v["data"]["kind"], "project");
    assert_eq!(v["data"]["identity"], "alpha");
    assert_eq!(v["data"]["details"]["changes"]["description"]["to"], "Research notes");
}
