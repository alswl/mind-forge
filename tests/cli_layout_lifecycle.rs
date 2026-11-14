use assert_cmd::Command;
use serde_json::Value;
use std::{fs, path::Path};

fn run(root: &Path, args: &[&str]) -> std::process::Output {
    Command::cargo_bin("mf").unwrap().current_dir(root).arg("--json").args(args).output().unwrap()
}
fn ok(root: &Path, args: &[&str]) -> Value {
    let output = run(root, args);
    assert!(output.status.success(), "{args:?}: {}", String::from_utf8_lossy(&output.stderr));
    serde_json::from_slice::<Value>(&output.stdout).unwrap()["data"].clone()
}
fn fail(root: &Path, args: &[&str], kind: &str) {
    let output = run(root, args);
    assert!(!output.status.success(), "unexpected success: {args:?}");
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"]["kind"], kind, "{error}");
}
fn write(root: &Path, path: &str, content: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}
fn setup() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    ok(root, &["init"]);
    ok(root, &["project", "new", "p", "--layout", "category"]);
    for name in ["brief", "reader", "empty"] {
        ok(root, &["mind", "new", name, "-p", "p", "--title", &format!("Title {name}")]);
    }
    for name in ["design", "summary"] {
        ok(root, &["output", "new", "brief", name, "--file", "-p", "p"]);
    }
    write(root, "projects/p/outputs/brief/design.md", "# Title\n\n![image](../../assets/brief/picture.png)\n");
    write(root, "projects/p/prompts/brief.md", "# Prompt\n");
    write(root, "projects/p/thinking/brief.md", "# Thinking\n");
    write(root, "projects/p/assets/brief/picture.png", "binary-data");
    write(root, "projects/p/sources/brief/evidence.md", "# Evidence\n");
    write(root, "projects/p/reviews/brief.md", "# Review\n");
    write(root, "projects/p/reviews/.gitkeep", "placeholder");
    write(root, "projects/p/evals/brief/result.json", "{}");
    write(root, "projects/p/outputs/reader/index.md", "[shared](../../sources/brief/evidence.md)\n");
    write(
        root,
        "projects/p/mind-index.yaml",
        "schema: '1'\nsources:\n  evidence:\n    path: sources/brief/evidence.md\n    name: evidence\nassets:\n  picture:\n    path: assets/brief/picture.png\n    name: picture\npublish_records:\n  record:\n    path: outputs/brief/design.md\n    target_name: local\n    status: published\n",
    );
    dir
}
fn tree(root: &Path) -> Vec<(String, Vec<u8>)> {
    let mut files = walkdir::WalkDir::new(root)
        .into_iter()
        .map(|e| e.unwrap())
        .filter(|e| e.file_type().is_file())
        .map(|e| (e.path().strip_prefix(root).unwrap().to_string_lossy().into_owned(), fs::read(e.path()).unwrap()))
        .collect::<Vec<_>>();
    files.sort();
    files
}

#[test]
fn bidirectional_full_conversion_and_dry_run() {
    let dir = setup();
    let root = dir.path();
    let before = tree(root);
    let preview = ok(root, &["lifecycle", "convert", "--to", "bundle", "-p", "p", "-n"]);
    assert!(preview["writes"].as_array().unwrap().len() > 10);
    assert_eq!(tree(root), before);
    assert!(!root.join(".mind-forge").exists());
    ok(root, &["lifecycle", "convert", "--to", "bundle", "-p", "p", "-y"]);
    for path in [
        "brief/outputs/design.md",
        "brief/outputs/summary.md",
        "brief/prompt.md",
        "brief/thinking.md",
        "brief/sources/evidence.md",
        "brief/assets/picture.png",
        "brief/reviews.md",
        "brief/evals/result.json",
        "reviews/.gitkeep",
    ] {
        assert!(root.join("projects/p").join(path).is_file(), "{path}");
    }
    assert_eq!(ok(root, &["mind", "list", "-p", "p"])["minds"].as_array().unwrap().len(), 3);
    assert!(
        fs::read_to_string(root.join("projects/p/reader/outputs/index.md"))
            .unwrap()
            .contains("../../brief/sources/evidence.md")
    );
    ok(root, &["lifecycle", "convert", "--to", "category", "-p", "p", "-y"]);
    for (path, bytes) in before {
        if !path.ends_with("mind.yaml") && !path.ends_with("mind-index.yaml") {
            assert_eq!(fs::read(root.join(&path)).unwrap(), bytes, "{path}");
        }
    }
    let index = fs::read_to_string(root.join("projects/p/mind-index.yaml")).unwrap();
    assert!(index.contains("path: sources/brief/evidence.md"));
    assert!(index.contains("path: outputs/brief/design.md"));
}

#[test]
fn mind_rename_moves_all_roles_preserves_titles_and_blocks_referenced_delete() {
    let dir = setup();
    let root = dir.path();
    fail(root, &["lifecycle", "mind-remove", "projects/p::brief", "-n"], "inbound_reference");
    ok(root, &["lifecycle", "mind-rename", "projects/p::brief", "renamed", "-y"]);
    assert_eq!(ok(root, &["mind", "show", "projects/p::renamed"])["title"], "Title brief");
    assert!(root.join("projects/p/reviews/renamed.md").is_file());
    assert!(
        fs::read_to_string(root.join("projects/p/outputs/reader/index.md"))
            .unwrap()
            .contains("sources/renamed/evidence.md")
    );
    fail(root, &["lifecycle", "mind-remove", "projects/p::renamed", "-y"], "inbound_reference");
    ok(root, &["lifecycle", "output-rename", "projects/p::renamed::summary", "short", "-y"]);
    ok(root, &["lifecycle", "output-remove", "projects/p::renamed::short", "-y"]);
    assert!(!root.join("projects/p/outputs/renamed/short.md").exists());
}

#[test]
fn saved_plans_refuse_new_inbound_files_and_new_additional() {
    for added in ["projects/p/outputs/reader/new.md", "projects/p/evals/brief/new.json"] {
        let dir = setup();
        let root = dir.path();
        let preview = ok(root, &["lifecycle", "convert", "--to", "bundle", "-p", "p", "-n", "--emit-plan"]);
        let saved = tempfile::NamedTempFile::new().unwrap();
        fs::write(saved.path(), serde_json::to_vec(&preview["plan"]).unwrap()).unwrap();
        write(root, added, "new data");
        let before = tree(root);
        fail(root, &["lifecycle", "apply", saved.path().to_str().unwrap(), "-y"], "stale_plan");
        assert_eq!(tree(root), before);
    }
}

#[test]
fn cross_project_move_transfers_yaml_authority_and_shared_links() {
    let dir = setup();
    let root = dir.path();
    ok(root, &["project", "new", "q", "--layout", "bundle"]);
    ok(root, &["lifecycle", "mind-move", "projects/p::brief", "--to-project", "q", "-y"]);
    assert!(root.join("projects/q/brief/outputs/design.md").is_file());
    assert!(root.join("projects/q/brief/reviews.md").is_file());
    let index = fs::read_to_string(root.join("projects/q/mind-index.yaml")).unwrap();
    assert!(index.contains("path: brief/sources/evidence.md"), "{index}");
    let reader = fs::read_to_string(root.join("projects/p/outputs/reader/index.md")).unwrap();
    assert!(reader.contains("../../../q/brief/sources/evidence.md"), "{reader}");
    assert_eq!(ok(root, &["mind", "show", "projects/q::brief"])["title"], "Title brief");
}

#[test]
fn every_writer_family_cooperates_with_root_lock_and_reads_gate_on_recovery() {
    use fs2::FileExt;
    let dir = setup();
    let root = dir.path();
    let lock = fs::File::open(root).unwrap();
    lock.try_lock_exclusive().unwrap();
    for args in [
        vec!["mind", "new", "blocked", "-p", "p"],
        vec!["output", "index", "-p", "p"],
        vec!["project", "index"],
        vec!["article", "index", "-p", "p"],
        vec!["source", "index", "-p", "p"],
        vec!["asset", "index", "-p", "p"],
        vec!["term", "new", "blocked"],
        vec!["build", "projects/p::brief::design"],
        vec!["publish", "run", "projects/p::brief::design"],
        vec!["config", "generate", "--out", "blocked.yaml"],
        vec!["lifecycle", "convert", "--to", "bundle", "-p", "p", "-y"],
    ] {
        fail(root, &args, "write_locked");
    }
    ok(root, &["mind", "list", "-p", "p"]);
    ok(root, &["lifecycle", "convert", "--to", "bundle", "-p", "p", "-n"]);
    FileExt::unlock(&lock).unwrap();
    let preview = ok(root, &["lifecycle", "convert", "--to", "bundle", "-p", "p", "-n", "--emit-plan"]);
    write(
        root,
        ".mind-forge/lifecycle/journal.json",
        &serde_json::json!({"plan":preview["plan"], "phase":"prepared"}).to_string(),
    );
    for args in [
        vec!["mind", "list", "-p", "p"],
        vec!["output", "show", "projects/p::brief::design"],
        vec!["source", "list", "-p", "p"],
        vec!["mind", "new", "blocked", "-p", "p"],
    ] {
        fail(root, &args, "recovery_required");
    }
    ok(root, &["lifecycle", "resume"]);
    ok(root, &["lifecycle", "resume"]);
    assert!(root.join("projects/p/brief/outputs/design.md").exists());
}

#[test]
fn initial_incomplete_journal_recovers_from_saved_plan_without_losing_evidence() {
    let dir = setup();
    let root = dir.path();
    let preview = ok(root, &["lifecycle", "convert", "--to", "bundle", "-p", "p", "-n", "--emit-plan"]);
    let saved = tempfile::NamedTempFile::new().unwrap();
    fs::write(saved.path(), preview["plan"].to_string()).unwrap();
    write(root, ".mind-forge/lifecycle/journal.json.next", "{\"plan\":");
    ok(root, &["lifecycle", "rollback", "--plan", saved.path().to_str().unwrap()]);
    ok(root, &["lifecycle", "rollback"]);
    assert!(root.join("projects/p/outputs/brief/design.md").exists());
    assert!(
        fs::read_dir(root.join(".mind-forge")).unwrap().any(|e| e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("lifecycle-preparation-"))
    );
}

#[test]
fn output_move_default_selection_and_internal_delete_are_consistent() {
    let dir = setup();
    let root = dir.path();
    let path = root.join("projects/p/mind.yaml");
    let mut config: serde_yaml::Value = serde_yaml::from_slice(&fs::read(&path).unwrap()).unwrap();
    config["minds"]["brief"]["default_output"] = "summary".into();
    fs::write(&path, serde_yaml::to_string(&config).unwrap()).unwrap();
    ok(root, &["lifecycle", "output-rename", "projects/p::brief::summary", "short", "-y"]);
    assert_eq!(ok(root, &["mind", "show", "projects/p::brief"])["default_output"], "short");
    ok(
        root,
        &["lifecycle", "output-move", "projects/p::brief::short", "--to-project", "p", "--to-mind", "reader", "-y"],
    );
    assert!(ok(root, &["mind", "show", "projects/p::brief"])["default_output"].is_null());
    ok(root, &["lifecycle", "mind-remove", "projects/p::reader", "-y"]);
    ok(root, &["lifecycle", "mind-remove", "projects/p::brief", "-y"]);
    assert!(ok(root, &["output", "list", "-p", "p"])["outputs"].as_array().unwrap().is_empty());
}

#[test]
fn both_output_shapes_and_hidden_role_content_survive_conversion() {
    let dir = setup();
    let root = dir.path();
    ok(root, &["output", "new", "brief", "blocks", "-p", "p"]);
    write(
        root,
        "projects/p/outputs/brief/blocks/02-body.md",
        "## Body\n\n![asset](../../../assets/brief/picture.png)\n",
    );
    write(root, "projects/p/outputs/brief/blocks/.keep", "hidden output content");
    write(root, "projects/p/evals/brief/.keep", "hidden additional content");
    let before = tree(root);
    ok(root, &["lifecycle", "convert", "--to", "bundle", "-p", "p", "-y"]);
    assert_eq!(ok(root, &["output", "show", "projects/p::brief::blocks"])["shape"], "directory");
    ok(root, &["lifecycle", "convert", "--to", "category", "-p", "p", "-y"]);
    for (path, bytes) in before {
        if !path.ends_with("mind.yaml") && !path.ends_with("mind-index.yaml") {
            assert_eq!(fs::read(root.join(path)).unwrap(), bytes);
        }
    }
}

#[test]
fn affected_unsupported_references_and_output_inbound_links_refuse_without_writes() {
    for reference in [
        "[x][ref]\n[ref]: ../brief/summary.md\n",
        "<a href='../brief/summary.md'>x</a>",
        "[x]({{base}}/brief/summary.md)",
    ] {
        let dir = setup();
        let root = dir.path();
        write(root, "projects/p/outputs/reader/refs.md", reference);
        let before = tree(root);
        let kind = if reference.contains("{{") { "reference_unsupported" } else { "inbound_reference" };
        fail(root, &["lifecycle", "output-remove", "projects/p::brief::summary", "-y"], kind);
        assert_eq!(tree(root), before);
    }
}

#[test]
fn conversion_conflicts_and_unknown_ownership_refuse_without_side_effects() {
    for path in ["projects/p/brief/collision.md", "projects/p/reviews/unowned.md", "projects/p/docs/old-article.md"] {
        let dir = setup();
        let root = dir.path();
        write(root, path, "unknown content");
        let before = tree(root);
        let output = run(root, &["lifecycle", "convert", "--to", "bundle", "-p", "p", "-y"]);
        assert!(!output.status.success(), "{path}");
        assert_eq!(tree(root), before);
    }
}

#[test]
fn bundle_container_placeholders_survive_both_conversion_directions() {
    let dir = setup();
    let root = dir.path();
    ok(root, &["lifecycle", "convert", "--to", "bundle", "-p", "p", "-y"]);
    write(root, "projects/p/brief/.gitkeep", "container placeholder");
    write(root, "projects/p/brief/.notes/draft.txt", "hidden content");
    ok(root, &["lifecycle", "convert", "--to", "category", "-p", "p", "-y"]);
    ok(root, &["lifecycle", "convert", "--to", "bundle", "-p", "p", "-y"]);
    assert_eq!(fs::read_to_string(root.join("projects/p/brief/.gitkeep")).unwrap(), "container placeholder");
    assert_eq!(fs::read_to_string(root.join("projects/p/brief/.notes/draft.txt")).unwrap(), "hidden content");
    assert!(root.join("projects/p/brief/outputs/design.md").is_file());
}

fn root_setup() -> tempfile::TempDir {
    let dir = setup();
    let root = dir.path();
    for entry in fs::read_dir(root.join("projects/p")).unwrap() {
        let entry = entry.unwrap();
        fs::rename(entry.path(), root.join(entry.file_name())).unwrap();
    }
    fs::remove_dir(root.join("projects/p")).unwrap();
    write(root, "minds.yaml", "schema: '1'\nprojects: [projects/child]\n");
    write(root, "projects/child/mind.yaml", "schema: '1'\ncontent_layout: bundle\n");
    ok(root, &["mind", "new", "nested", "-p", "child"]);
    dir
}

#[test]
fn root_project_conversion_and_lifecycle_preserve_child_ownership() {
    let dir = root_setup();
    let root = dir.path();
    ok(root, &["output", "new", "brief", "chapters", "-p", "."]);
    let child = tree(&root.join("projects/child"));
    let before = tree(root);
    ok(root, &["lifecycle", "convert", "--to", "bundle", "-p", ".", "-n"]);
    assert_eq!(tree(root), before);
    for mode in ["bundle", "category", "bundle"] {
        ok(root, &["lifecycle", "convert", "--to", mode, "-p", ".", "-y"]);
        assert_eq!(tree(&root.join("projects/child")), child);
        assert_eq!(ok(root, &["mind", "list", "-p", "."])["minds"].as_array().unwrap().len(), 3);
    }
    for path in [
        "brief/outputs/design.md",
        "brief/outputs/summary.md",
        "brief/prompt.md",
        "brief/thinking.md",
        "brief/sources/evidence.md",
        "brief/assets/picture.png",
        "brief/reviews.md",
        "brief/evals/result.json",
    ] {
        assert!(root.join(path).is_file(), "{path}");
    }
    fail(root, &["lifecycle", "mind-remove", "::brief", "-y"], "inbound_reference");
    ok(root, &["lifecycle", "mind-rename", "::brief", "renamed", "-y"]);
    assert_eq!(ok(root, &["mind", "show", "::renamed"])["title"], "Title brief");
    ok(root, &["lifecycle", "output-rename", "::renamed::summary", "short", "-y"]);
    ok(root, &["lifecycle", "output-remove", "::renamed::short", "-y"]);
    ok(root, &["lifecycle", "mind-move", "::renamed", "--to-project", "child", "-y"]);
    assert!(root.join("projects/child/renamed/sources/evidence.md").is_file());
    ok(root, &["lifecycle", "mind-move", "projects/child::renamed", "--to-project", ".", "-y"]);
    assert!(root.join("renamed/sources/evidence.md").is_file());
    let index = fs::read_to_string(root.join("mind-index.yaml")).unwrap();
    assert!(index.contains("path: renamed/sources/evidence.md"), "{index}");
    ok(root, &["lifecycle", "mind-remove", "::empty", "-y"]);
}

#[test]
fn default_convert_preparation_failures_recover_without_external_plan() {
    for stage in ["namespace", "directory", "partial", "backup", "publish"] {
        for recovery in ["resume", "rollback"] {
            let dir = setup();
            let root = dir.path();
            let authored = tree(&root.join("projects"));
            let output = Command::cargo_bin("mf")
                .unwrap()
                .current_dir(root)
                .arg("--json")
                .args(["lifecycle", "convert", "--to", "bundle", "-p", "p", "--yes"])
                .env("MF_TEST_LIFECYCLE_PREPARE_FAILURE", stage)
                .output()
                .unwrap();
            assert!(!output.status.success(), "{stage}");
            assert!(String::from_utf8_lossy(&output.stderr).contains("injected_failure"));
            assert_eq!(tree(&root.join("projects")), authored);
            if stage != "namespace" {
                ok(root, &["lifecycle", recovery]);
                ok(root, &["lifecycle", recovery]);
                ok(root, &["mind", "list", "-p", "p"]);
            }
            let completed = recovery == "resume" && matches!(stage, "backup" | "publish");
            if !completed {
                assert_eq!(tree(&root.join("projects")), authored);
                ok(root, &["lifecycle", "convert", "--to", "bundle", "-p", "p", "-y"]);
            }
            assert!(root.join("projects/p/brief/outputs/design.md").is_file());
        }
    }
}

#[test]
fn partial_preparation_abort_preserves_later_edits() {
    let dir = setup();
    let root = dir.path();
    let output = Command::cargo_bin("mf")
        .unwrap()
        .current_dir(root)
        .args(["lifecycle", "convert", "--to", "bundle", "-p", "p", "-y"])
        .env("MF_TEST_LIFECYCLE_PREPARE_FAILURE", "partial")
        .output()
        .unwrap();
    assert!(!output.status.success());
    write(root, "projects/p/prompts/brief.md", "later edit");
    ok(root, &["lifecycle", "resume"]);
    assert_eq!(fs::read_to_string(root.join("projects/p/prompts/brief.md")).unwrap(), "later edit");
    assert!(root.join(".mind-forge/lifecycle-preparation-evidence-0/journal.json.next").exists());
    ok(root, &["lifecycle", "convert", "--to", "bundle", "-p", "p", "-y"]);
    assert_eq!(fs::read_to_string(root.join("projects/p/brief/prompt.md")).unwrap(), "later edit");
}

#[test]
fn custom_legacy_root_config_does_not_override_new_model_core_paths() {
    for populated in [false, true] {
        let dir = setup();
        let root = dir.path();
        let path = root.join("projects/p/mind.yaml");
        let mut config = fs::read_to_string(&path).unwrap();
        config.push_str("\nlayout:\n  articles: legacy-docs\n  sources: legacy-sources\n  assets: legacy-assets\n");
        fs::write(path, config).unwrap();
        if populated {
            write(root, "projects/p/legacy-sources/unknown.md", "unassigned legacy source");
            let before = tree(root);
            fail(root, &["lifecycle", "convert", "--to", "bundle", "-p", "p", "-y"], "ownership_unresolved");
            assert_eq!(tree(root), before);
        } else {
            for mode in ["bundle", "category"] {
                ok(root, &["lifecycle", "convert", "--to", mode, "-p", "p", "-y"]);
            }
            assert!(root.join("projects/p/sources/brief/evidence.md").exists());
        }
    }
}

#[test]
fn root_project_stale_plan_and_output_moves() {
    let dir = root_setup();
    let root = dir.path();
    let preview = ok(root, &["lifecycle", "convert", "--to", "bundle", "-p", ".", "-n", "--emit-plan"]);
    let saved = tempfile::NamedTempFile::new().unwrap();
    fs::write(saved.path(), serde_json::to_vec(&preview["plan"]).unwrap()).unwrap();
    write(root, "reviews/brief-extra.md", "new unknown owner");
    fail(root, &["lifecycle", "apply", saved.path().to_str().unwrap(), "-y"], "stale_plan");
    fs::remove_file(root.join("reviews/brief-extra.md")).unwrap();
    ok(root, &["lifecycle", "output-move", "::brief::summary", "--to-project", "child", "--to-mind", "nested", "-y"]);
    assert!(root.join("projects/child/nested/outputs/summary.md").is_file());
    ok(
        root,
        &[
            "lifecycle",
            "output-move",
            "projects/child::nested::summary",
            "--to-project",
            ".",
            "--to-mind",
            "brief",
            "-y",
        ],
    );
    assert!(root.join("outputs/brief/summary.md").is_file());
}

#[test]
fn prepared_default_cli_recovery_protects_later_edits() {
    for stage in ["backup", "publish"] {
        let dir = root_setup();
        let root = dir.path();
        let output = Command::cargo_bin("mf")
            .unwrap()
            .current_dir(root)
            .args(["lifecycle", "convert", "--to", "bundle", "-p", ".", "-y"])
            .env("MF_TEST_LIFECYCLE_PREPARE_FAILURE", stage)
            .output()
            .unwrap();
        assert!(!output.status.success());
        write(root, "prompts/brief.md", "later edit");
        fail(root, &["lifecycle", "resume"], "recovery_conflict");
        assert_eq!(fs::read_to_string(root.join("prompts/brief.md")).unwrap(), "later edit");
    }
}
