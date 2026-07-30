use assert_cmd::Command;

mod common;

fn setup() -> common::TempDir {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    repo
}

fn mf(repo: &common::TempDir) -> Command {
    let mut command = Command::cargo_bin("mf").unwrap();
    command.args(["--root", repo.path().to_str().unwrap(), "--project", "alpha"]);
    command
}

#[test]
fn unsupported_index_schema_is_usage_error_before_registration() {
    let repo = setup();
    let project = repo.path().join("alpha");
    std::fs::write(project.join("mind-index.yaml"), "schema: '99'\nsources: []\n").unwrap();
    std::fs::create_dir_all(project.join("sources/file")).unwrap();
    let input = project.join("sources/file/existing.md");
    std::fs::write(&input, "existing\n").unwrap();
    let output = mf(&repo).args(["source", "new", input.to_str().unwrap(), "--register-only"]).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("incompatible schema") || stderr.contains("upgrade"), "{stderr}");
    assert_eq!(std::fs::read_to_string(project.join("mind-index.yaml")).unwrap(), "schema: '99'\nsources: []\n");
}

#[test]
fn register_only_preserves_terms_publish_records_and_unknown_source_kind() {
    let repo = setup();
    let project = repo.path().join("alpha");
    std::fs::write(
        project.join("mind-index.yaml"),
        "schema: '1'\nsources:\n  old:\n    type: file\n    path: sources/file/old.md\n    source_kind: article_prompt\nterms:\n  - term: stable\n    pinyin: stable\n    definition: keep\npublish_records:\n  - path: docs/post.md\n    target_name: local\n    status: draft\n",
    )
    .unwrap();
    std::fs::create_dir_all(project.join("sources/file")).unwrap();
    let input = project.join("sources/file/new.md");
    std::fs::write(&input, "new\n").unwrap();
    let output = mf(&repo)
        .args(["source", "new", input.to_str().unwrap(), "--register-only", "--name", "new"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let index = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index.contains("article_prompt"));
    assert!(index.contains("stable"));
    assert!(index.contains("publish_records"));
    assert!(index.contains("name: new"));
}
