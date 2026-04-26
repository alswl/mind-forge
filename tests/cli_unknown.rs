use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn unknown_command_returns_usage_error() {
    Command::cargo_bin("mf")
        .expect("binary exists")
        .arg("sourse")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("unrecognized subcommand"));
}

#[test]
fn missing_required_argument_returns_usage_error() {
    Command::cargo_bin("mf")
        .expect("binary exists")
        .args(["source", "add"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("required arguments were not provided"));
}
