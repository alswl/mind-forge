use assert_cmd::Command;

mod common;

#[test]
fn update_article_status() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    common::write_article_index(&repo, "alpha", "launch-plan");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "article",
            "update",
            "launch-plan",
            "--status",
            "published",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let index_yaml = std::fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert!(index_yaml.contains("status: published"), "mind-index.yaml: {index_yaml}");
}

#[test]
fn update_article_dry_run_does_not_write() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    common::write_article_index(&repo, "alpha", "launch-plan");
    let before = std::fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "article",
            "update",
            "launch-plan",
            "--status",
            "published",
            "--project",
            "alpha",
            "--dry-run",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let after = std::fs::read_to_string(repo.path().join("alpha/mind-index.yaml")).unwrap();
    assert_eq!(after, before);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("[dry-run] would update article: docs/launch-plan.md"));
}

#[test]
fn update_article_json_envelope() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    common::write_article_index(&repo, "alpha", "launch-plan");

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "--format",
            "json",
            "article",
            "update",
            "docs/launch-plan.md",
            "--status",
            "published",
            "--project",
            "alpha",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    assert_eq!(v["data"]["kind"], "article");
    assert_eq!(v["data"]["identity"], "docs/launch-plan.md");
    assert_eq!(v["data"]["details"]["changes"]["status"]["to"], "published");
}
