use assert_cmd::Command;
use insta::assert_snapshot;

fn run(args: &[&str]) -> (String, String, i32) {
    let output =
        Command::cargo_bin("mf").expect("binary exists").args(args).output().expect("command runs");
    (
        String::from_utf8(output.stdout).expect("stdout utf8"),
        String::from_utf8(output.stderr).expect("stderr utf8"),
        output.status.code().unwrap_or_default(),
    )
}

#[test]
fn top_level_help_snapshot() {
    let (stdout, _, code) = run(&["--help"]);
    assert_eq!(code, 0);
    assert_snapshot!("top_level_help", stdout);
}

#[test]
fn group_help_snapshot() {
    let (stdout, _, code) = run(&["source", "--help"]);
    assert_eq!(code, 0);
    assert_snapshot!("source_help", stdout);
}

#[test]
fn leaf_help_snapshot() {
    let (stdout, _, code) = run(&["article", "publish", "--help"]);
    assert_eq!(code, 0);
    assert_snapshot!("article_publish_help", stdout);
}

#[test]
fn version_works() {
    let (stdout, _, code) = run(&["--version"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("mf 0.1.0"));
}
