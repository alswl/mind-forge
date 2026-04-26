use assert_cmd::Command;
use tempfile::NamedTempFile;

#[test]
fn all_leaf_commands_return_placeholder_or_successful_output_shape() {
    let asset_file = NamedTempFile::new().expect("temp file");
    let asset_path = asset_file.path().to_string_lossy().to_string();

    let cases: Vec<Vec<String>> = vec![
        vec!["source", "list"],
        vec!["source", "add", "https://example.com/feed.xml"],
        vec!["source", "update", "--all"],
        vec!["asset", "list"],
        vec!["asset", "add", asset_path.as_str()],
        vec!["asset", "update", "--all"],
        vec!["project", "new", "demo", "--force"],
        vec!["project", "list"],
        vec!["project", "lint"],
        vec!["article", "new", "Hello"],
        vec!["article", "list"],
        vec!["article", "build"],
        vec!["article", "publish", "post-1"],
        vec!["term", "list"],
        vec!["term", "new", "CLI"],
        vec!["term", "fix", "--all"],
    ]
    .into_iter()
    .map(|items| items.into_iter().map(str::to_string).collect())
    .collect();

    for case in cases {
        let output = Command::cargo_bin("mf")
            .expect("binary exists")
            .args(&case)
            .output()
            .expect("command runs");
        assert_eq!(output.status.code(), Some(64), "case: {case:?}");
        let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
        assert!(stdout.contains("[not implemented]"), "case: {case:?}, stdout: {stdout}");
    }
}

#[test]
fn completion_is_real_command() {
    Command::cargo_bin("mf").expect("binary exists").args(["completion", "zsh"]).assert().code(0);
}
