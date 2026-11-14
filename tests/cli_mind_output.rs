use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::path::Path;

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
fn fail(root: &Path, args: &[&str]) -> Value {
    let out = run(root, args);
    assert!(!out.status.success(), "unexpected success {args:?}");
    serde_json::from_slice::<Value>(&out.stderr).or_else(|_| serde_json::from_slice(&out.stdout)).unwrap()
}
fn setup() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    ok(dir.path(), &["init"]);
    dir
}

#[test]
fn zero_one_many_outputs_both_layouts_and_shapes() {
    let repo = setup();
    let root = repo.path();
    for mode in ["category", "bundle"] {
        ok(root, &["project", "new", mode, "--layout", mode]);
        let project = root.join("projects").join(mode);
        let created = ok(root, &["mind", "new", "brief", "-p", mode]);
        let mind_id = created["identity"].as_str().unwrap();
        let mind = ok(root, &["mind", "show", mind_id]);
        assert_eq!(mind["outputs"].as_array().unwrap().len(), 0);
        assert!(fail(root, &["build", mind_id]).to_string().contains("no output"));
        assert!(!project.join("build").exists());
        let listed = ok(root, &["mind", "list", "-p", mode]);
        assert_eq!(listed["minds"][0]["identity"], mind_id);
        ok(root, &["project", "lint", "-p", mode]);
        for (name, file) in [("design", false), ("summary", true)] {
            let mut args = vec!["output", "new", mind_id, name];
            if file {
                args.push("--file");
            }
            let created = ok(root, &args);
            let id = created["identity"].as_str().unwrap();
            let output = ok(root, &["output", "show", id]);
            let relative = output["path"].as_str().unwrap();
            let expected =
                if mode == "category" { format!("outputs/brief/{name}") } else { format!("brief/outputs/{name}") };
            assert_eq!(relative, if file { format!("{expected}.md") } else { expected });
            let source = if file { project.join(relative) } else { project.join(relative).join("01-opening.md") };
            fs::write(&source, format!("# {name}\n\nVisible {mode}\n\n> [!mf-private]\n> secret-{mode}\n")).unwrap();
            if !file {
                fs::write(
                    project.join(relative).join("02-private.md"),
                    "---\nmind-forge-visibility: private\n---\nprivate block\n",
                )
                .unwrap();
            }
            let original = fs::read(&source).unwrap();
            let build = ok(root, &["build", id]);
            assert_eq!(build["location"]["identity"], id);
            let artifact = project.join(format!("build/brief/{name}.md"));
            let rendered = fs::read_to_string(&artifact).unwrap();
            assert!(rendered.contains(&format!("Visible {mode}")));
            assert!(!rendered.contains("secret"));
            assert!(!rendered.contains("private block"));
            assert_eq!(fs::read(&source).unwrap(), original);
            fail(root, &["build", id, "--out", source.to_str().unwrap()]);
            assert_eq!(fs::read(&source).unwrap(), original);
            if name == "design" {
                ok(root, &["build", mind_id]);
            }
        }
        let ambiguous = fail(root, &["build", mind_id]);
        assert_eq!(ambiguous["error"]["kind"], "selection_ambiguous");
        assert_eq!(ambiguous["error"]["candidates"].as_array().unwrap().len(), 2);
        let outputs = ok(root, &["output", "list", mind_id]);
        for output in outputs["outputs"].as_array().unwrap() {
            assert_eq!(ok(root, &["output", "show", output["identity"].as_str().unwrap()]), *output);
        }
        ok(root, &["output", "index", "-p", mode]);
        ok(root, &["project", "index"]);
        let index: serde_yaml::Value =
            serde_yaml::from_str(&fs::read_to_string(project.join("mind-index.yaml")).unwrap()).unwrap();
        assert_eq!(index["outputs"].as_sequence().unwrap().len(), 2);
        assert!(index.get("articles").is_none());
        let mut config: serde_yaml::Value =
            serde_yaml::from_str(&fs::read_to_string(project.join("mind.yaml")).unwrap()).unwrap();
        config["minds"]["brief"]["default_output"] = "summary".into();
        fs::write(project.join("mind.yaml"), serde_yaml::to_string(&config).unwrap()).unwrap();
        assert!(ok(root, &["build", mind_id])["output"].as_str().unwrap().ends_with("build/brief/summary.md"));
        config["minds"]["brief"]["default_output"] = "missing".into();
        fs::write(project.join("mind.yaml"), serde_yaml::to_string(&config).unwrap()).unwrap();
        fail(root, &["build", mind_id]);
    }
}

#[test]
fn dry_run_read_only_conflicts_and_ambiguity() {
    let repo = setup();
    let root = repo.path();
    ok(root, &["project", "new", "c", "--layout", "category"]);
    let project = root.join("projects/c");
    let original = fs::read(project.join("mind.yaml")).unwrap();
    ok(root, &["mind", "new", "empty", "-p", "c", "--dry-run"]);
    assert_eq!(fs::read(project.join("mind.yaml")).unwrap(), original);
    assert!(!project.join("outputs").exists());
    for name in ["alpha", "beta"] {
        ok(root, &["mind", "new", name, "-p", "c"]);
        ok(root, &["output", "new", name, "summary", "-p", "c", "--dry-run"]);
        assert!(!project.join("outputs").exists() || name == "beta");
        ok(root, &["output", "new", name, "summary", "-p", "c", "--file"]);
    }
    let err = fail(root, &["output", "show", "summary", "-p", "c"]);
    assert_eq!(err["error"]["kind"], "selection_ambiguous");
    fail(root, &["mind", "new", "ALPHA", "-p", "c"]);
    fail(root, &["mind", "new", "build", "-p", "c"]);
    fail(root, &["mind", "new", "../outside", "-p", "c"]);
    fail(root, &["output", "new", "alpha", "../outside", "-p", "c"]);
    fs::create_dir_all(project.join("outputs/alpha/summary")).unwrap();
    let err = fail(root, &["output", "list", "-p", "c"]);
    assert_eq!(err["error"]["kind"], "shape_conflict");
}

#[test]
fn exact_identity_project_mismatch_and_role_discovery() {
    let repo = setup();
    let root = repo.path();
    for mode in ["category", "bundle"] {
        ok(root, &["project", "new", mode, "--layout", mode]);
        let project = root.join(format!("projects/{mode}"));
        let prompt = if mode == "category" { "prompts/notes.md" } else { "notes/prompt.md" };
        fs::create_dir_all(project.join(prompt).parent().unwrap()).unwrap();
        fs::write(project.join(prompt), "# Intent\n").unwrap();
        let m = ok(root, &["mind", "show", "notes", "-p", mode]);
        assert_eq!(m["roles"]["prompt"], prompt);
        assert_eq!(m["outputs"].as_array().unwrap().len(), 0);
    }
    fail(root, &["mind", "show", "projects/category::notes", "-p", "bundle"]);
}

#[test]
fn publish_uses_output_location() {
    let repo = setup();
    let root = repo.path();
    for mode in ["category", "bundle"] {
        ok(root, &["project", "new", mode, "--layout", mode]);
        let project = root.join(format!("projects/{mode}"));
        let mut config: serde_yaml::Value =
            serde_yaml::from_str(&fs::read_to_string(project.join("mind.yaml")).unwrap()).unwrap();
        config["publish"] = serde_yaml::from_str(
            "targets:\n  - name: test-local\n    type: local\n    config:\n      path: build/deliveries\n",
        )
        .unwrap();
        fs::write(project.join("mind.yaml"), serde_yaml::to_string(&config).unwrap()).unwrap();
        for mind in ["alpha", "beta"] {
            ok(root, &["mind", "new", mind, "-p", mode]);
            let created = ok(root, &["output", "new", mind, "summary", "--file", "-p", mode]);
            let id = created["identity"].as_str().unwrap();
            ok(root, &["build", id]);
            let pub_result = ok(root, &["publish", "run", id, "--target", "test-local", "--dry-run"]);
            assert!(pub_result.to_string().contains(&format!("build/{mind}/summary.md")), "{pub_result}");
            assert!(pub_result.to_string().contains(&format!("{mind}/summary.md")));
            ok(root, &["publish", "update", id, "--target", "test-local", "--set", "status=published"]);
        }
        let index: serde_yaml::Value =
            serde_yaml::from_str(&fs::read_to_string(project.join("mind-index.yaml")).unwrap()).unwrap();
        assert_eq!(index["publish_records"].as_sequence().unwrap().len(), 2);
    }
}

fn tree(root: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    walkdir::WalkDir::new(root)
        .into_iter()
        .map(|e| e.unwrap())
        .filter(|e| e.path() != root)
        .map(|e| {
            (
                e.path().strip_prefix(root).unwrap().to_string_lossy().into_owned(),
                if e.file_type().is_file() { fs::read(e.path()).unwrap() } else { vec![] },
            )
        })
        .collect()
}

#[test]
fn previews_and_queries_are_byte_preserving_for_registered_paths() {
    let repo = setup();
    let root = repo.path();
    ok(root, &["project", "new", "nested/team/research", "--layout", "bundle"]);
    let project = "nested/team/research";
    ok(root, &["mind", "new", "主题", "-p", project]);
    let created = ok(root, &["output", "new", "主题", "摘要", "--file", "-p", project]);
    let identity = created["identity"].as_str().unwrap();
    assert_eq!(identity, "nested/team/research::主题::摘要");
    let before = tree(root);
    ok(root, &["mind", "new", "next", "-p", project, "--dry-run"]);
    ok(root, &["output", "new", "主题", "next", "-p", project, "--dry-run"]);
    ok(root, &["mind", "list", "-p", project]);
    ok(root, &["output", "show", identity]);
    ok(root, &["output", "index", "-p", project, "--dry-run"]);
    ok(root, &["build", identity, "--dry-run"]);
    assert_eq!(tree(root), before, "preview and reads must not add even directories/locks/caches");
    ok(root, &["project", "index"]);
    assert!(root.join(project).join("mind-index.yaml").is_file());
    assert!(fs::read_to_string(root.join("minds.yaml")).unwrap().contains(project));
    let show = ok(root, &["project", "show", project]);
    assert_eq!(show["mind_count"], 1);
    assert_eq!(show["output_count"], 1);
}

#[test]
fn legacy_ownership_is_not_inferred_and_short_names_do_not_shadow_outputs() {
    let repo = setup();
    let root = repo.path();
    ok(root, &["project", "new", "legacy"]);
    let before = tree(root);
    fail(root, &["mind", "new", "invented", "-p", "legacy"]);
    fail(root, &["project", "new", "legacy", "--layout", "bundle", "--force"]);
    assert_eq!(tree(root), before);
    ok(root, &["project", "new", "modern", "--layout", "category"]);
    let before_new = tree(root);
    fail(root, &["article", "new", "wrong-subject", "-p", "modern"]);
    assert_eq!(tree(root), before_new);
    for mind in ["brief", "other"] {
        ok(root, &["mind", "new", mind, "-p", "modern"]);
    }
    ok(root, &["output", "new", "brief", "summary", "--file", "-p", "modern"]);
    ok(root, &["output", "new", "other", "brief", "--file", "-p", "modern"]);
    let error = fail(root, &["build", "brief", "-p", "modern"]);
    assert_eq!(error["error"]["kind"], "selection_ambiguous");
    ok(root, &["build", "projects/modern::brief"]);
}
