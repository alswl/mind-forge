use assert_cmd::Command;
use insta::assert_snapshot;

fn run(args: &[&str]) -> (String, String, i32) {
    let output = Command::cargo_bin("mf").expect("binary exists").args(args).output().expect("command runs");
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
    let (stdout, _, code) = run(&["build", "--help"]);
    assert_eq!(code, 0);
    assert_snapshot!("build_help", stdout);
}

#[test]
fn version_works() {
    let (stdout, _, code) = run(&["--version"]);
    assert_eq!(code, 0);
    assert!(stdout.contains(&format!("mf {}", env!("CARGO_PKG_VERSION"))));
}

#[test]
fn config_help_snapshot() {
    let (stdout, _, code) = run(&["config", "--help"]);
    assert_eq!(code, 0);
    assert_snapshot!("config_help", stdout);
}

#[test]
fn config_terminal_help_snapshot() {
    let (stdout, _, code) = run(&["config", "terminal", "--help"]);
    assert_eq!(code, 0);
    assert_snapshot!("config_terminal_help", stdout);
}

#[test]
fn project_help_snapshot() {
    let (stdout, _, code) = run(&["project", "--help"]);
    assert_eq!(code, 0);
    assert_snapshot!("project_help", stdout);
}

#[test]
fn project_remove_help_snapshot() {
    let (stdout, _, code) = run(&["project", "remove", "--help"]);
    assert_eq!(code, 0);
    assert_snapshot!("project_remove_help", stdout);
}

#[test]
fn publish_help_snapshot() {
    let (stdout, _, code) = run(&["publish", "--help"]);
    assert_eq!(code, 0);
    assert_snapshot!("publish_help", stdout);
}

#[test]
fn asset_help_snapshot() {
    let (stdout, _, code) = run(&["asset", "--help"]);
    assert_eq!(code, 0);
    assert_snapshot!("asset_help", stdout);
}

#[test]
fn asset_list_help_snapshot() {
    let (stdout, _, code) = run(&["asset", "list", "--help"]);
    assert_eq!(code, 0);
    assert_snapshot!("asset_list_help", stdout);
}

#[test]
fn asset_update_help_snapshot() {
    let (stdout, _, code) = run(&["asset", "update", "--help"]);
    assert_eq!(code, 0);
    assert_snapshot!("asset_update_help", stdout);
}

#[test]
fn asset_index_help_snapshot() {
    let (stdout, _, code) = run(&["asset", "index", "--help"]);
    assert_eq!(code, 0);
    assert_snapshot!("asset_index_help", stdout);
}

#[test]
fn asset_remove_help_snapshot() {
    let (stdout, _, code) = run(&["asset", "remove", "--help"]);
    assert_eq!(code, 0);
    assert_snapshot!("asset_remove_help", stdout);
}

#[test]
fn source_list_help_snapshot() {
    let (stdout, _, code) = run(&["source", "list", "--help"]);
    assert_eq!(code, 0);
    assert_snapshot!("source_list_help", stdout);
}

#[test]
fn source_update_help_snapshot() {
    let (stdout, _, code) = run(&["source", "update", "--help"]);
    assert_eq!(code, 0);
    assert_snapshot!("source_update_help", stdout);
}

#[test]
fn source_index_help_snapshot() {
    let (stdout, _, code) = run(&["source", "index", "--help"]);
    assert_eq!(code, 0);
    assert_snapshot!("source_index_help", stdout);
}

#[test]
fn source_remove_help_snapshot() {
    let (stdout, _, code) = run(&["source", "remove", "--help"]);
    assert_eq!(code, 0);
    assert_snapshot!("source_remove_help", stdout);
}

#[test]
fn source_clean_help_snapshot() {
    let (stdout, _, code) = run(&["source", "clean", "--help"]);
    assert_eq!(code, 0);
    assert_snapshot!("source_clean_help", stdout);
}

// ── Term help snapshots (012-term-core) ─────────────────────────────────

#[test]
fn term_help_snapshot() {
    let (stdout, _, code) = run(&["term", "--help"]);
    assert_eq!(code, 0);
    assert_snapshot!("term_help", stdout);
}

#[test]
fn term_new_help_snapshot() {
    let (stdout, _, code) = run(&["term", "new", "--help"]);
    assert_eq!(code, 0);
    assert_snapshot!("term_new_help", stdout);
}

#[test]
fn term_list_help_snapshot() {
    let (stdout, _, code) = run(&["term", "list", "--help"]);
    assert_eq!(code, 0);
    assert_snapshot!("term_list_help", stdout);
}

#[test]
fn term_lint_help_snapshot() {
    let (stdout, _, code) = run(&["term", "lint", "--help"]);
    assert_eq!(code, 0);
    assert_snapshot!("term_lint_help", stdout);
}

#[test]
fn term_fix_help_snapshot() {
    let (stdout, _, code) = run(&["term", "fix", "--help"]);
    assert_eq!(code, 0);
    assert_snapshot!("term_fix_help", stdout);
}

#[test]
fn term_remove_help_snapshot() {
    let (stdout, _, code) = run(&["term", "remove", "--help"]);
    assert_eq!(code, 0);
    assert_snapshot!("term_remove_help", stdout);
}

// ── Publish target help snapshots ─────────────────

#[test]
fn publish_target_help_snapshot() {
    let (stdout, _, code) = run(&["publish", "target", "--help"]);
    assert_eq!(code, 0);
    assert_snapshot!("publish_target_help", stdout);
}

#[test]
fn publish_target_list_help_snapshot() {
    let (stdout, _, code) = run(&["publish", "target", "list", "--help"]);
    assert_eq!(code, 0);
    assert_snapshot!("publish_target_list_help", stdout);
}

// ── Render help snapshots (020-render-output) ─────────────────

#[test]
fn render_help_snapshot() {
    let (stdout, _, code) = run(&["render", "--help"]);
    assert_eq!(code, 0);
    assert_snapshot!("render_help", stdout);
}

#[test]
fn article_help_snapshot() {
    let (stdout, _, code) = run(&["article", "--help"]);
    assert_eq!(code, 0);
    assert_snapshot!("article_help", stdout);
}

#[test]
fn article_new_help_snapshot() {
    let (stdout, _, code) = run(&["article", "new", "--help"]);
    assert_eq!(code, 0);
    assert_snapshot!("article_new_help", stdout);
}

#[test]
fn article_remove_help_snapshot() {
    let (stdout, _, code) = run(&["article", "remove", "--help"]);
    assert_eq!(code, 0);
    assert_snapshot!("article_remove_help", stdout);
}

#[test]
fn article_convert_help_snapshot() {
    let (stdout, _, code) = run(&["article", "convert", "--help"]);
    assert_eq!(code, 0);
    assert_snapshot!("article_convert_help", stdout);
}

#[test]
fn render_template_list_help_snapshot() {
    let (stdout, _, code) = run(&["render", "template", "list", "--help"]);
    assert_eq!(code, 0);
    assert_snapshot!("render_template_list_help", stdout);
}

// ── Init help snapshots ──

#[test]
fn init_help_snapshot() {
    let (stdout, _, code) = run(&["init", "--help"]);
    assert_eq!(code, 0);
    assert_snapshot!("init_help", stdout);
}
