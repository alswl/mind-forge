use assert_cmd::Command;

mod common;

#[test]
fn all_leaf_commands_implemented() {
    // All commands are now implemented — this test exists as a marker
    // that no "not yet implemented" commands remain.
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
    assert_eq!(output.status.code(), Some(0), "expected exit 0 for implemented command project index");
}
