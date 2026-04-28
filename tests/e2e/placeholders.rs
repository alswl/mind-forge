use crate::datasets::Dataset;
use crate::helpers::*;

// ---------------------------------------------------------------------------
// 所有叶命令输出 placeholder
// ---------------------------------------------------------------------------

macro_rules! vec_strings {
    ($($x:expr),*) => (vec![$($x.to_string()),*]);
}

fn assert_placeholder(args: &[&str], repo_dir: &std::path::Path) {
    let (stdout, stderr, code) = run_in(repo_dir, args);
    assert_eq!(code, 64, "expected placeholder exit 64 for args {args:?}");
    assert!(stderr.is_empty(), "stderr should be empty for placeholder: {stderr}");
    assert!(stdout.contains("[not implemented]"), "args {args:?}, stdout: {stdout}");
}

/// E2E: 所有已定义但未实现的叶命令都输出 [not implemented] placeholder
#[test]
fn all_leaf_commands_are_placeholders() {
    let ds = Dataset::empty();

    let cases: Vec<Vec<String>> = vec![
        vec_strings!["source", "list"],
        vec_strings!["source", "add", "placeholder.pdf", "--type", "file"],
        vec_strings!["source", "update", "placeholder.pdf"],
        vec_strings!["source", "index"],
        vec_strings!["source", "remove", "placeholder.pdf"],
        vec_strings!["source", "clean"],
        vec_strings!["asset", "list"],
        vec_strings!["asset", "add", "placeholder.pdf"],
        vec_strings!["asset", "update", "placeholder.pdf"],
        vec_strings!["asset", "index"],
        vec_strings!["project", "new", "demo", "--force"],
        vec_strings!["project", "list"],
        vec_strings!["project", "archive", "demo"],
        vec_strings!["project", "status", "demo"],
        vec_strings!["project", "lint"],
        vec_strings!["project", "index"],
        vec_strings!["article", "new", "Hello"],
        vec_strings!["article", "list"],
        vec_strings!["article", "lint"],
        vec_strings!["article", "index"],
        vec_strings!["term", "list"],
        vec_strings!["term", "new", "CLI"],
        vec_strings!["term", "lint"],
        vec_strings!["term", "learn", "--original", "cli", "--correct", "CLI"],
        vec_strings!["term", "fix", "CLI"],
    ];

    for case in &cases {
        let args: Vec<&str> = case.iter().map(String::as_str).collect();
        assert_placeholder(&args, ds.root());
    }
}

/// E2E: JSON 格式的 placeholder 包含 status/command/args
#[test]
fn json_placeholder_structure() {
    let ds = Dataset::empty();

    let (stdout, _, _) = run_in(ds.root(), &["--format", "json", "source", "list"]);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["status"], "not_implemented");
    assert_eq!(parsed["command"], "mf source list");
    assert!(parsed.get("args").is_some(), "JSON should contain args field");
}
