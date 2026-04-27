use assert_cmd::Command;

macro_rules! vec_strings {
    ($($x:expr),*) => (vec![$($x.to_string()),*]);
}

fn assert_exit_64(args: &[&str]) {
    let output =
        Command::cargo_bin("mf").expect("binary exists").args(args).output().expect("command runs");
    assert_eq!(
        output.status.code(),
        Some(64),
        "expected exit 64 for args {args:?}, got {:?}",
        output.status.code()
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("[not implemented]"), "args {args:?}, stdout: {stdout}");
}

#[test]
fn all_leaf_commands_return_placeholder() {
    let cases: Vec<Vec<String>> = vec![
        // source
        vec_strings!["source", "list"],
        vec_strings!["source", "add", "placeholder.pdf", "--type", "file"],
        vec_strings!["source", "update", "placeholder.pdf"],
        vec_strings!["source", "index"],
        vec_strings!["source", "remove", "placeholder.pdf"],
        vec_strings!["source", "clean"],
        // asset
        vec_strings!["asset", "list"],
        vec_strings!["asset", "add", "placeholder.pdf"],
        vec_strings!["asset", "update", "placeholder.pdf"],
        vec_strings!["asset", "index"],
        // project
        vec_strings!["project", "new", "demo", "--force"],
        vec_strings!["project", "list"],
        vec_strings!["project", "archive", "demo"],
        vec_strings!["project", "status", "demo"],
        vec_strings!["project", "lint"],
        vec_strings!["project", "index"],
        // article
        vec_strings!["article", "new", "Hello"],
        vec_strings!["article", "list"],
        vec_strings!["article", "lint"],
        vec_strings!["article", "index"],
        // term
        vec_strings!["term", "list"],
        vec_strings!["term", "new", "CLI"],
        vec_strings!["term", "lint"],
        vec_strings!["term", "learn", "--original", "cli", "--correct", "CLI"],
        vec_strings!["term", "fix", "CLI"],
        // build
        vec_strings!["build", "article-name"],
        // publish
        vec_strings!["publish", "run", "docs/foo.md"],
        vec_strings!["publish", "update", "docs/foo.md"],
        // config
        vec_strings!["config", "schema"],
        vec_strings!["config", "show"],
        vec_strings!["config", "init"],
    ];

    for case in &cases {
        let args: Vec<&str> = case.iter().map(String::as_str).collect();
        assert_exit_64(&args);
    }
}

#[test]
fn completion_is_real_command() {
    Command::cargo_bin("mf").expect("binary exists").args(["completion", "zsh"]).assert().code(0);
}

#[test]
fn build_invalid_article_returns_usage_error() {
    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .args(["build", ""])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(2), "empty article should be usage error");
    let stderr = String::from_utf8(output.stderr).expect("stderr utf8");
    assert!(stderr.contains("mf article list"), "hint should mention mf article list");
}

#[test]
fn unknown_flag_returns_usage_error() {
    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .args(["term", "list", "--nonexistent"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(2));
}
