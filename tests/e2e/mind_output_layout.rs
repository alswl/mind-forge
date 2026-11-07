//! A local-only authored-output lifecycle; all data lives in TempDir.
use super::helpers::{TempDir, run_in};
use serde_json::Value;
use std::fs;
use std::path::Path;

fn command(root: &Path, args: &[&str]) -> Value {
    let mut all = vec!["--json"];
    all.extend_from_slice(args);
    let (stdout, stderr, code) = run_in(root, &all);
    assert_eq!(code, 0, "{args:?}: {stdout} {stderr}");
    serde_json::from_str::<Value>(&stdout).unwrap()["data"].clone()
}

#[test]
fn same_repo_mixed_layout_local_publish_preserves_ownership_and_source() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    command(root, &["init"]);
    for layout in ["category", "bundle"] {
        command(root, &["project", "new", layout, "--layout", layout]);
        let project = root.join(format!("projects/{layout}"));
        let mut cfg: serde_yaml::Value =
            serde_yaml::from_str(&fs::read_to_string(project.join("mind.yaml")).unwrap()).unwrap();
        cfg["publish"] = serde_yaml::from_str(
            "targets:\n  - name: local-test\n    type: local\n    config:\n      path: build/delivered\n",
        )
        .unwrap();
        fs::write(project.join("mind.yaml"), serde_yaml::to_string(&cfg).unwrap()).unwrap();
        command(root, &["mind", "new", "brief", "-p", layout]);
        for (output, file) in [("design", false), ("summary", true)] {
            let mut args = vec!["output", "new", "brief", output, "-p", layout];
            if file {
                args.push("--file");
            }
            let new = command(root, &args);
            let identity = new["identity"].as_str().unwrap();
            let source = project.join(new["path"].as_str().unwrap());
            let source = if file { source } else { source.join("01-opening.md") };
            let text =
                format!("# {output}\n\n{layout}-{output}-public\n\n> [!mf-private]\n> hidden-{layout}-{output}\n");
            fs::write(&source, &text).unwrap();
            command(root, &["build", identity]);
            let published = command(root, &["publish", "run", identity, "--target", "local-test"]);
            assert_eq!(published["location"]["identity"], identity);
            let artifact = fs::read_to_string(project.join(format!("build/delivered/brief/{output}.md"))).unwrap();
            assert!(artifact.contains(&format!("{layout}-{output}-public")));
            assert!(!artifact.contains("hidden-"));
            assert_eq!(fs::read_to_string(source).unwrap(), text);
            command(root, &["publish", "update", identity, "--target", "local-test", "--set", "status=published"]);
        }
        assert_eq!(command(root, &["mind", "list", "-p", layout])["minds"].as_array().unwrap().len(), 1);
    }
}
