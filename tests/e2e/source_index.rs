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

    let (stdout, stderr, code) = run_in(ds.root(), &["--format", "json", "source", "list", "--project", "alpha"]);
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
