//! Source management is file/YAML backed and exposes no corpus commands.

use assert_cmd::Command;

mod common;

#[test]
fn corpus_commands_and_source_indexing_flags_are_rejected() {
    let repo = common::setup_repo();
    for args in [
        vec!["search", "query"],
        vec!["source", "search", "query"],
        vec!["source", "sync"],
        vec!["source", "status"],
        vec!["source", "admin"],
        vec!["source", "export"],
        vec!["source", "import"],
        vec!["source", "trace"],
        vec!["source", "advanced"],
        vec!["source", "new", "reference.md", "--no-index"],
        vec!["source", "new", "reference.md", "--article", "docs/post.md"],
    ] {
        let output = Command::cargo_bin("mf").unwrap().current_dir(repo.path()).args(&args).output().unwrap();
        assert_eq!(output.status.code(), Some(2), "{args:?}: {}", String::from_utf8_lossy(&output.stderr));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("unexpected") || stderr.contains("unrecognized"), "{args:?}: {stderr}");
    }
}

#[test]
fn new_source_persists_file_and_registration_without_creating_a_corpus() {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    std::fs::write(repo.path().join("reference.md"), "# Evidence\n").unwrap();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .current_dir(repo.path())
        .args(["--json", "source", "new", "reference.md", "-p", "alpha"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let details = &result["data"]["details"];
    for field in ["rag_indexed", "indexing", "registration_key", "projection_degraded"] {
        assert!(details.get(field).is_none(), "unexpected {field}: {result}");
    }
    let project = repo.path().join("alpha");
    let index = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index.contains("reference"), "{index}");
    let copied = project.join(result["data"]["path"].as_str().unwrap());
    assert_eq!(std::fs::read_to_string(copied).unwrap(), "# Evidence\n");
    assert!(!repo.path().join(".mind-forge/cache").exists());
}
