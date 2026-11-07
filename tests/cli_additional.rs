use assert_cmd::Command;
use serde_json::Value;
use std::{fs, path::Path};

fn run(root: &Path, args: &[&str]) -> std::process::Output {
    Command::cargo_bin("mf").unwrap().current_dir(root).arg("--json").args(args).output().unwrap()
}
fn ok(root: &Path, args: &[&str]) -> Value {
    let out = run(root, args);
    assert!(
        out.status.success(),
        "{args:?}: {} {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice::<Value>(&out.stdout).unwrap()["data"].clone()
}
fn fail(root: &Path, args: &[&str], kind: &str) -> Value {
    let out = run(root, args);
    assert!(!out.status.success(), "{args:?}");
    let value: Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(value["error"]["kind"], kind, "{value}");
    value["error"].clone()
}
fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}
fn tree(root: &Path) -> Vec<(String, Vec<u8>)> {
    let mut entries = walkdir::WalkDir::new(root)
        .into_iter()
        .map(|e| e.unwrap())
        .map(|e| {
            (
                e.path().strip_prefix(root).unwrap().to_string_lossy().into_owned(),
                if e.file_type().is_file() { fs::read(e.path()).unwrap() } else { vec![] },
            )
        })
        .collect::<Vec<_>>();
    entries.sort();
    entries
}
fn setup(mode: &str, project: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "minds.yaml", "schema: '1'\nprojects: ['.', './nested/q']\n");
    for p in ["", "nested/q"] {
        write(&root.join(p), "mind.yaml", &format!("content_layout: {mode}\nminds:\n  a: {{}}\n  b: {{}}\n"));
    }
    let p = root.join(project);
    for (kind, file) in [("unregistered-kind", true), ("evals", false)] {
        let path = resource(mode, kind, "a", file);
        write(&p, &if file { path } else { format!("{path}/nested/reviews.md") }, "author content\n");
    }
    dir
}
fn resource(mode: &str, kind: &str, owner: &str, file: bool) -> String {
    let suffix = if file { ".md" } else { "" };
    if mode == "bundle" { format!("{owner}/{kind}{suffix}") } else { format!("{kind}/{owner}{suffix}") }
}

#[test]
fn both_layouts_root_nested_files_directories_roundtrip_and_transaction() {
    for mode in ["category", "bundle"] {
        for project in ["", "nested/q"] {
            let dir = setup(mode, project);
            let root = dir.path();
            let p = root.join(project);
            let before = tree(root);
            let data = ok(root, &["additional", "list", "-p", p.to_str().unwrap()]);
            assert_eq!(data["additional"].as_array().unwrap().len(), 2);
            for item in data["additional"].as_array().unwrap() {
                let id = item["identity"].as_str().unwrap();
                assert_eq!(ok(root, &["additional", "show", id]), *item);
                assert_eq!(
                    ok(root, &["additional", "show", item["path"].as_str().unwrap(), "-p", p.to_str().unwrap()]),
                    *item
                );
            }
            assert_eq!(before, tree(root));
            for (kind, file) in [("unregistered-kind", true), ("evals", false)] {
                let old = resource(mode, kind, "a", file);
                let new = resource(mode, &format!("{kind}-renamed"), "a", file);
                let leaf = if file { old.clone() } else { format!("{old}/nested/reviews.md") };
                write(
                    &p,
                    "reader.md",
                    &format!("[inline]({leaf}#anchor)\n[x][ref]\n[ref]: {leaf}\n<img src=\"{leaf}\">\n"),
                );
                let before = tree(root);
                let preview =
                    ok(root, &["additional", "rename", &old, &new, "-p", p.to_str().unwrap(), "-n", "--emit-plan"]);
                assert_eq!(preview["kind"], "lifecycle");
                assert_eq!(before, tree(root));
                ok(root, &["additional", "rename", &old, &new, "-p", p.to_str().unwrap(), "-y"]);
                assert!(!p.join(&old).exists());
                assert!(p.join(&new).exists());
                assert!(fs::read_to_string(p.join("reader.md")).unwrap().contains(&new));
                let before = tree(root);
                fail(root, &["additional", "remove", &new, "-p", p.to_str().unwrap(), "-y"], "inbound_reference");
                assert_eq!(before, tree(root));
                fs::remove_file(p.join("reader.md")).unwrap();
                let before = tree(root);
                ok(root, &["additional", "remove", &new, "-p", p.to_str().unwrap(), "-n"]);
                assert_eq!(before, tree(root));
                ok(root, &["additional", "remove", &new, "-p", p.to_str().unwrap(), "-y"]);
                assert!(!p.join(&new).exists());
                ok(root, &["mind", "index", "--check", "-p", p.to_str().unwrap()]);
            }
            assert_eq!(
                fs::read_to_string(root.join(if project.is_empty() { "nested/q/mind.yaml" } else { "mind.yaml" }))
                    .unwrap(),
                format!("content_layout: {mode}\nminds:\n  a: {{}}\n  b: {{}}\n")
            );
        }
    }
}

#[test]
fn ambiguous_selectors_and_structural_conflicts_return_candidates() {
    for mode in ["category", "bundle"] {
        let dir = setup(mode, "");
        let root = dir.path();
        write(root, &resource(mode, "evals", "b", true), "review b");
        let err = fail(root, &["additional", "show", "evals", "-p", root.to_str().unwrap()], "selection_ambiguous");
        assert_eq!(err["candidates"].as_array().unwrap().len(), 2);
        write(root, &resource(mode, "evals", "a", true), "collision");
        let err = fail(root, &["additional", "list", "-p", root.to_str().unwrap()], "path_conflict");
        assert_eq!(err["candidates"].as_array().unwrap().len(), 2);
    }
}

#[test]
fn case_collision_unknown_owner_and_unsafe_paths_are_refused() {
    for mode in ["category", "bundle"] {
        let dir = setup(mode, "");
        let root = dir.path();
        write(root, &resource(mode, "EVALS", "a", true), "case collision");
        fail(root, &["additional", "list", "-p", root.to_str().unwrap()], "path_conflict");
    }
    let dir = setup("category", "");
    let root = dir.path();
    write(root, "reviews/unknown.md", "unknown owner");
    fail(root, &["additional", "list", "-p", root.to_str().unwrap()], "ownership_unresolved");
    fs::remove_file(root.join("reviews/unknown.md")).unwrap();
    let before = tree(root);
    fail(root, &["additional", "show", "../escape", "-p", root.to_str().unwrap()], "unsafe_path");
    fail(root, &["additional", "rename", "evals/a", "../escape", "-p", root.to_str().unwrap(), "-y"], "usage");
    assert_eq!(before, tree(root));
}

#[test]
fn core_reserved_hidden_tool_roots_and_placeholders_are_excluded() {
    for mode in ["category", "bundle"] {
        let dir = setup(mode, "");
        let root = dir.path();
        for base in
            ["build", "templates", "cache", "target", "node_modules", "vendor", "dist", ".agent", "custom-build"]
        {
            let path = if mode == "bundle" { format!("a/{base}/a.md") } else { format!("{base}/a.md") };
            write(root, &path, "excluded");
            write(root, &format!("{base}/arbitrary.md"), "excluded root");
        }
        write(
            root,
            "mind.yaml",
            &format!("content_layout: {mode}\nminds:\n  a: {{}}\nlayout:\n  build_output: custom-build\n"),
        );
        write(root, &resource(mode, if mode == "bundle" { "prompt" } else { "prompts" }, "a", true), "core");
        write(root, &resource(mode, "thinking", "a", true), "core");
        write(root, &format!("{}/evidence.md", resource(mode, "sources", "a", false)), "core");
        write(root, &format!("{}/.gitkeep", resource(mode, "evals", "a", false)), "placeholder");
        let items = ok(root, &["additional", "list", "-p", root.to_str().unwrap()]);
        assert_eq!(items["additional"].as_array().unwrap().len(), 2, "{items}");
        fail(
            root,
            &[
                "additional",
                "rename",
                &resource(mode, "evals", "a", false),
                &resource(mode, "sources", "a", false),
                "-p",
                root.to_str().unwrap(),
                "-n",
            ],
            "invalid_additional_target",
        );
    }
}

#[test]
fn index_check_reports_differences_and_bindings_without_deciding_or_writing() {
    for mode in ["category", "bundle"] {
        let dir = setup(mode, "");
        let root = dir.path();
        let before = tree(root);
        let out = run(root, &["mind", "index", "--check", "-p", root.to_str().unwrap()]);
        assert_eq!(out.status.code(), Some(1));
        let report: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(report["data"]["kind"], "content_check");
        assert_eq!(report["data"]["issues"].as_array().unwrap().len(), 2);
        ok(root, &["mind", "index", "-n", "-p", root.to_str().unwrap()]);
        assert_eq!(tree(root), before);
        ok(root, &["mind", "index", "-p", root.to_str().unwrap()]);
        ok(root, &["output", "index", "--check", "-p", root.to_str().unwrap()]);
        let path = root.join("mind-index.yaml");
        let mut index: serde_yaml::Value = serde_yaml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        index["additional"][0]["mind_identity"] = "::wrong".into();
        index["additional"]
            .as_sequence_mut()
            .unwrap()
            .push(serde_yaml::from_str("identity: stale\npath: absent.md").unwrap());
        index["sources"] = serde_yaml::from_str("bad:\n  path: absent.md\nescape:\n  path: ../external\n").unwrap();
        fs::write(&path, serde_yaml::to_string(&index).unwrap()).unwrap();
        let before = tree(root);
        let out = run(root, &["mind", "index", "--check", "-p", root.to_str().unwrap()]);
        assert_eq!(out.status.code(), Some(1));
        let data: Value = serde_json::from_slice(&out.stdout).unwrap();
        let kinds: Vec<_> =
            data["data"]["issues"].as_array().unwrap().iter().map(|v| v["kind"].as_str().unwrap()).collect();
        for kind in ["projection_mismatch", "stale_projection", "missing_binding_target", "unsafe_binding"] {
            assert!(kinds.contains(&kind), "{data}");
        }
        assert_eq!(tree(root), before);
        assert_eq!(out.stdout, run(root, &["mind", "index", "--check", "-p", root.to_str().unwrap()]).stdout);
        ok(root, &["mind", "index", "-p", root.to_str().unwrap()]);
        assert!(root.join(resource(mode, "unregistered-kind", "a", true)).is_file());
    }
}

#[test]
fn new_resources_invalidate_saved_plans_and_recovery_reuses_lifecycle() {
    let dir = setup("bundle", "");
    let root = dir.path();
    let preview =
        ok(root, &["additional", "rename", "a/evals", "a/results", "-p", root.to_str().unwrap(), "-n", "--emit-plan"]);
    let plan = tempfile::NamedTempFile::new().unwrap();
    fs::write(plan.path(), serde_json::to_vec(&preview["plan"]).unwrap()).unwrap();
    write(root, "a/new-kind.md", "new data");
    fail(root, &["lifecycle", "apply", plan.path().to_str().unwrap(), "-y"], "stale_plan");
    fs::remove_file(root.join("a/new-kind.md")).unwrap();
    ok(root, &["lifecycle", "apply", plan.path().to_str().unwrap(), "-y"]);
    assert!(root.join("a/results").exists());
    ok(root, &["lifecycle", "resume"]);
    assert!(root.join("a/results").exists());
    fail(root, &["lifecycle", "rollback"], "transaction_finished");
}

#[cfg(unix)]
#[test]
fn external_symlink_is_never_followed() {
    let dir = setup("bundle", "");
    let root = dir.path();
    let outside = tempfile::tempdir().unwrap();
    write(outside.path(), "secret.md", "untouched");
    std::os::unix::fs::symlink(outside.path(), root.join("a/external")).unwrap();
    fail(root, &["additional", "list", "-p", root.to_str().unwrap()], "unsafe_path");
    assert_eq!(fs::read_to_string(outside.path().join("secret.md")).unwrap(), "untouched");
}

#[cfg(unix)]
#[test]
fn read_discovery_isolates_child_projects_and_ignores_tool_symlinks() {
    let dir = setup("bundle", "");
    let root = dir.path();
    let outside = tempfile::tempdir().unwrap();
    for path in ["node_modules", ".hidden", "a/node_modules", "nested/q/unsafe"] {
        std::os::unix::fs::symlink(outside.path(), root.join(path)).unwrap();
    }
    let before = tree(outside.path());
    assert_eq!(
        ok(root, &["additional", "list", "-p", root.to_str().unwrap()])["additional"].as_array().unwrap().len(),
        2
    );
    assert_eq!(tree(outside.path()), before);
}

#[test]
fn rename_one_resource_leaves_other_minds_in_the_same_category() {
    let dir = setup("category", "");
    let root = dir.path();
    write(root, "evals/b/stay.md", "other mind\n");
    ok(root, &["additional", "rename", "evals/a", "results/a", "-p", root.to_str().unwrap(), "-y"]);
    assert_eq!(fs::read_to_string(root.join("evals/b/stay.md")).unwrap(), "other mind\n");
    assert!(root.join("results/a/nested/reviews.md").exists());
    fail(
        root,
        &["additional", "rename", "results/a", "results/b", "-p", root.to_str().unwrap(), "-n"],
        "invalid_additional_target",
    );
}

#[test]
fn binding_check_exposes_default_role_owner_and_duplicate_errors() {
    let dir = setup("category", "");
    let root = dir.path();
    write(root, "mind.yaml", "content_layout: category\nminds:\n  a:\n    default_output: absent\n  b: {}\n");
    write(root, "sources/a/e.md", "evidence");
    write(root, "outputs/a/main.md", "source text");
    write(
        root,
        "mind-index.yaml",
        "schema: '1'\nsources:\n  first:\n    path: sources/a/e.md\n    mind_identity: '::b'\n  second:\n    path: sources/a/e.md\n  wrong-role:\n    path: outputs/a/main.md\n",
    );
    let before = tree(root);
    let out = run(root, &["mind", "index", "--check", "-p", root.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    let data: Value = serde_json::from_slice(&out.stdout).unwrap();
    let issues = data["data"]["issues"].as_array().unwrap();
    for kind in ["missing_default_output", "binding_mismatch", "duplicate_binding"] {
        assert!(issues.iter().any(|v| v["kind"] == kind), "{data}");
    }
    assert_eq!(issues.iter().filter(|v| v["kind"] == "binding_mismatch").count(), 2);
    assert_eq!(tree(root), before);
}

#[test]
fn removing_projection_does_not_remove_disk_and_core_case_is_not_additional() {
    let dir = setup("bundle", "");
    let root = dir.path();
    ok(root, &["mind", "index", "-p", root.to_str().unwrap()]);
    write(root, "mind-index.yaml", "schema: '1'\nadditional: []\noutputs: []\n");
    assert_eq!(
        ok(root, &["additional", "list", "-p", root.to_str().unwrap()])["additional"].as_array().unwrap().len(),
        2
    );
    let out = run(root, &["mind", "index", "--check", "-p", root.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    write(root, "a/Sources/e.md", "core collision");
    fail(root, &["additional", "list", "-p", root.to_str().unwrap()], "path_conflict");
}
