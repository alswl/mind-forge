use assert_cmd::Command;
use tempfile::TempDir;

mod common;

fn setup() -> (common::TempDir, TempDir, std::path::PathBuf) {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");

    let source_dir = TempDir::new().unwrap();
    let source = source_dir.path().join("sample.png");
    std::fs::write(&source, b"fake png content").unwrap();

    (repo, source_dir, source)
}

fn mf(repo: &common::TempDir) -> Command {
    let mut cmd = Command::cargo_bin("mf").unwrap();
    cmd.args(["--root", repo.path().to_str().unwrap(), "--project", "alpha"]);
    cmd
}

#[test]
fn asset_new_copies_file_and_indexes_entry() {
    let (repo, _source_dir, source) = setup();

    let output = mf(&repo).args(["asset", "new", source.to_str().unwrap()]).output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert!(String::from_utf8_lossy(&output.stderr).is_empty(), "new form should not warn");

    let project = repo.path().join("alpha");
    assert!(project.join("assets/sample.png").exists(), "asset file should be copied");

    let index = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index.contains("sample.png"), "index should contain sample.png: {index}");
}

#[test]
fn asset_add_legacy_alias_warns_and_surfaces_json_warning() {
    let (repo, _source_dir, source) = setup();

    let output = mf(&repo).args(["--json", "asset", "add", source.to_str().unwrap()]).output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("WARN:"), "legacy add must warn: {stderr}");
    assert!(stderr.contains("`asset add` is deprecated; use `mf asset new"), "stderr: {stderr}");

    let v: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid JSON envelope");
    let warnings = v["data"]["warnings"].as_array().expect("data.warnings array");
    assert!(
        warnings.iter().any(|w| w.as_str().unwrap_or("").contains("`asset add` is deprecated")),
        "data.warnings must include deprecation warning: {warnings:?}"
    );

    assert!(repo.path().join("alpha/assets/sample.png").exists(), "legacy add still applies the write");
}
