use assert_cmd::Command;

mod common;

fn assert_exit_64(args: &[&str], repo: &common::TempDir) {
    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path())
        .args(args)
        .output()
        .expect("command runs");
    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64 for args {args:?}, got {:?}",
        output.status.code()
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    let stderr = String::from_utf8(output.stderr).expect("stderr utf8");
    let all = stdout + &stderr;
    assert!(
        all.contains("[not implemented]") || all.contains("not yet implemented"),
        "args {args:?}, stdout+stderr: {all}"
    );
}

macro_rules! vec_strings {
    ($($x:expr),*) => (vec![$($x.to_string()),*]);
}

#[test]
fn all_leaf_commands_return_placeholder() {
    let repo = common::setup_repo();
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
        // project new/list/status/lint 已实现为真实命令
        // project archive 仍为 placeholder（exit 64 通过 not-implemented 错误）
        vec_strings!["project", "archive", "demo"],
        // project index 已实现为真实命令
        // article new/list/index/lint 已实现
        vec_strings!["term", "list"],
        vec_strings!["term", "new", "CLI"],
        vec_strings!["term", "lint"],
        vec_strings!["term", "learn", "--original", "cli", "--correct", "CLI"],
        vec_strings!["term", "fix", "CLI"],
        // build 已实现
        vec_strings!["publish", "run", "docs/foo.md"],
        vec_strings!["publish", "update", "docs/foo.md"],
        // config 已实现
    ];

    for case in &cases {
        let args: Vec<&str> = case.iter().map(String::as_str).collect();
        assert_exit_64(&args, &repo);
    }
}

#[test]
fn completion_is_real_command() {
    Command::cargo_bin("mf").expect("binary exists").args(["completion", "zsh"]).assert().code(0);
}

#[test]
fn unknown_flag_returns_usage_error() {
    let repo = common::setup_repo();
    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path())
        .args(["term", "list", "--nonexistent"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn project_index_returns_success() {
    let repo = common::setup_repo();
    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(repo.path())
        .args(["project", "index"])
        .output()
        .expect("command runs");
    assert_eq!(
        output.status.code(),
        Some(0),
        "expected exit 0 for implemented command project index"
    );
}
