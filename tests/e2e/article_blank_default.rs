use std::fs;

use crate::helpers;

/// T022: Create an arch directory article and a blank directory article,
/// then verify the concatenated block content matches the template semantics.
#[test]
fn directory_article_block_content_roundtrip() {
    let repo = helpers::TempDir::new().unwrap();
    // Establish mind repo context
    fs::write(repo.path().join("minds.yaml"), "schema_version: '1'\nprojects_dir: '.'\nprojects: []\n").unwrap();
    let project = repo.path().join("demo");
    fs::create_dir_all(&project).unwrap();
    fs::write(project.join("mind.yaml"), "schema_version: '1'\n").unwrap();

    // 1. Create arch directory article
    let (_, _, code) = helpers::run_in(&project, &["article", "new", "Roundtrip", "--template", "arch"]);
    assert_eq!(code, 0);
    let dir = project.join("docs/roundtrip");
    assert!(dir.is_dir());

    // 2. Concat blocks in filename order
    let mut names: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    let mut concat = String::new();
    for name in &names {
        concat.push_str(&fs::read_to_string(dir.join(name)).unwrap());
    }

    // 3. Verify all H2 sections from ARCH template
    assert!(concat.contains("# Roundtrip"));
    assert!(concat.contains("> Created:"));
    assert!(concat.contains("## Context"));
    assert!(concat.contains("## Decision"));
    assert!(concat.contains("## Consequence"));
    assert!(concat.contains("## Alternatives Considered"));
    assert_eq!(names.len(), 5, "expected 5 files: head + 4 H2 blocks");

    // 4. Create blank directory article
    let (_, _, code) = helpers::run_in(&project, &["article", "new", "Simple"]);
    assert_eq!(code, 0);
    let dir2 = project.join("docs/simple");
    assert!(dir2.is_dir());
    let head = fs::read_to_string(dir2.join("00-head.md")).unwrap();
    assert!(head.contains("# Simple"));
    assert!(head.contains("> Created:"));
}
