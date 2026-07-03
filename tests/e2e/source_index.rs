use std::fs;

use serde_json::Value;

use crate::datasets::Dataset;
use crate::helpers::run_in;

#[test]
fn source_index_discovers_existing_yuque_files() {
    let ds = Dataset::empty().with_standard_project("alpha");
    let project = ds.root().join("projects/alpha");

    fs::create_dir_all(project.join("sources/yuque/2025-05")).expect("create yuque source dir");
    fs::write(project.join("sources/yuque/2025-05/2025-05.md"), "# May report\n").expect("write source file");

    let (stdout, stderr, code) = run_in(ds.root(), &["source", "index", "--project", "alpha"]);
    assert_eq!(code, 0, "source index failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    assert!(stdout.contains("+1"), "source index should report one added source: {stdout}");

    let (stdout, stderr, code) = run_in(ds.root(), &["--output", "json", "source", "list", "--project", "alpha"]);
    assert_eq!(code, 0, "source list failed\nstdout:\n{stdout}\nstderr:\n{stderr}");

    let envelope: Value = serde_json::from_str(&stdout).expect("source list stdout should be json");
    let sources = envelope["data"]["sources"].as_array().expect("data.sources should be an array");
    assert!(
        sources
            .iter()
            .any(|source| { source["path"] == "sources/yuque/2025-05/2025-05.md" && source["source_kind"] == "yuque" }),
        "source list should include indexed yuque file: {stdout}"
    );

    let index_content = fs::read_to_string(project.join("mind-index.yaml")).expect("read mind-index.yaml");
    assert!(index_content.contains("sources/yuque/2025-05/2025-05.md"));
    assert!(index_content.contains("source_kind: yuque"), "source_kind should be persisted: {index_content}");
}

#[test]
fn register_only_then_reconcile_retains_existing_file() {
    let ds = Dataset::empty().with_standard_project("alpha");
    let project = ds.root().join("projects/alpha");
    fs::create_dir_all(project.join("sources/file")).unwrap();
    let file = project.join("sources/file/synthetic.md");
    fs::write(&file, "synthetic source\n").unwrap();

    let (_, stderr, code) =
        run_in(ds.root(), &["source", "new", file.to_str().unwrap(), "--project", "alpha", "--register-only"]);
    assert_eq!(code, 0, "register-only failed: {stderr}");

    let (_, stderr, code) = run_in(ds.root(), &["source", "index", "--project", "alpha"]);
    assert_eq!(code, 0, "reconcile failed: {stderr}");
    let index = fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    let yaml: serde_yaml::Value = serde_yaml::from_str(&index).unwrap();
    let matching = match &yaml["sources"] {
        serde_yaml::Value::Sequence(sources) => {
            sources.iter().filter(|source| source["path"].as_str() == Some("sources/file/synthetic.md")).count()
        }
        serde_yaml::Value::Mapping(sources) => {
            sources.values().filter(|source| source["path"].as_str() == Some("sources/file/synthetic.md")).count()
        }
        other => panic!("unexpected sources shape: {other:?}"),
    };
    assert_eq!(matching, 1, "registered path must remain unique: {index}");
    assert_eq!(fs::read_to_string(file).unwrap(), "synthetic source\n");
}
