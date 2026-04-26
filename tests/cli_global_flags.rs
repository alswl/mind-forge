use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn verbose_and_quiet_conflict() {
    Command::cargo_bin("mf")
        .expect("binary exists")
        .args(["--verbose", "--quiet", "source", "list"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("'--verbose' cannot be used with '--quiet'"));
}

#[test]
fn json_placeholder_uses_json_shape() {
    Command::cargo_bin("mf")
        .expect("binary exists")
        .args(["--format", "json", "source", "list"])
        .assert()
        .code(64)
        .stdout(predicate::str::contains("\"status\": \"not_implemented\""))
        .stdout(predicate::str::contains("\"command\": \"mf source list\""));
}

#[test]
fn explicit_config_path_is_accepted() {
    Command::cargo_bin("mf")
        .expect("binary exists")
        .args(["--config", "/tmp/mf-test.toml", "term", "list"])
        .assert()
        .code(64)
        .stdout(predicate::str::contains("[not implemented] mf term list"));
}
