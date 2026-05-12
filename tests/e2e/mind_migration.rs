use std::fs;

use crate::datasets::*;
use crate::helpers::*;

/// T128: mind 用户 10+ 命令迁移链路 E2E 测试
///
/// 模拟 mind 用户把 `mind` 替换成 `mf` 后最常用的命令序列。
/// 覆盖 A 类 alias、B1 新命令、B2 thin alias、C 类双签名。
#[test]
fn e2e_mind_user_migration_chain() {
    let ds = Dataset::empty();

    // ---- 1. mf project ls (A 类 alias: ls → list) ----
    let (stdout, stderr, code) = run_in(ds.root(), &["project", "ls"]);
    assert_eq!(code, 0, "project ls failed: {stderr}");
    assert!(stderr.is_empty(), "A-class alias should have clean stderr: {stderr}");
    assert!(!stdout.is_empty(), "should list projects");

    // ---- 2. mf project new alpha (create project) ----
    let (_stdout, stderr, code) = run_in(ds.root(), &["project", "new", "alpha"]);
    assert_eq!(code, 0, "project new failed: {stderr}");

    // ---- 3. mf project show alpha (B1 新命令) ----
    let (stdout, stderr, code) = run_in(ds.root(), &["project", "show", "alpha"]);
    assert_eq!(code, 0, "project show failed: {stderr}");
    assert!(stdout.contains("alpha"), "show output should contain project name: {stdout}");

    // ---- 4. mf terms list -p alpha (A 类 alias: terms → term) ----
    let (_stdout, stderr, code) = run_in(ds.root(), &["terms", "list", "-p", "alpha"]);
    assert_eq!(code, 0, "terms list failed: {stderr}");
    assert!(stderr.is_empty(), "terms alias should have clean stderr: {stderr}");

    // ---- 5. mf --json source list -p alpha (A 类: --json) ----
    let (stdout, stderr, code) = run_in(ds.root(), &["--json", "source", "list", "-p", "alpha"]);
    assert_eq!(code, 0, "--json source list failed: {stderr}");
    assert!(stdout.contains("\"status\": \"ok\""), "--json output should have envelope: {stdout}");

    // ---- 6. mf config compile (B2 thin alias, works at repo level) ----
    let (compile_stdout, stderr, code) = run_in(ds.root(), &["config", "compile"]);
    assert_eq!(code, 0, "config compile failed: {stderr}");

    // Compare with config show (byte-identical)
    let (show_stdout, stderr_show, code_show) = run_in(ds.root(), &["config", "show"]);
    assert_eq!(code_show, 0, "config show failed: {stderr_show}");
    assert_eq!(compile_stdout.trim(), show_stdout.trim(), "compile should match show output");

    // ---- 7. mf config default (B2 thin alias) ----
    let (stdout, stderr, code) = run_in(ds.root(), &["config", "default"]);
    assert_eq!(code, 0, "config default failed: {stderr}");
    assert!(!stdout.is_empty(), "config default should produce output");

    // ---- 8. mf --format json asset ls -p alpha (A 类: ls alias, -p short flag) ----
    let (stdout, stderr, code) = run_in(ds.root(), &["--format", "json", "asset", "ls", "-p", "alpha"]);
    assert_eq!(code, 0, "asset ls failed: {stderr}");
    assert!(stdout.contains("\"status\": \"ok\""), "json output should have envelope: {stdout}");

    // ---- 9. mf source add with --source-kind (C 类 primary form) ----
    // Place source file outside the project's sources/ directory
    let source_file = ds.root().join("external-source.md");
    fs::write(&source_file, b"source content").unwrap();

    let (_stdout, stderr, code) = run_in(
        ds.root(),
        &["source", "add", "--name", "test-source", "--source-kind", "yuque", "external-source.md", "-p", "alpha"],
    );
    assert_eq!(code, 0, "source add failed: {stderr}");
    assert!(!stderr.contains("[deprecated]"), "primary form should not warn: {stderr}");

    // ---- 10. mf source list -p alpha (verify source was added) ----
    let (stdout, stderr, code) = run_in(ds.root(), &["--format", "json", "source", "list", "-p", "alpha"]);
    assert_eq!(code, 0, "source list failed: {stderr}");
    assert!(stdout.contains("test-source"), "source list should show added source: {stdout}");

    // ---- 11. mf term new "API" --definition "..." --alias ap-i (C 类 primary form) ----
    let (_stdout, stderr, code) = run_in(
        ds.root(),
        &["term", "new", "API", "--definition", "Application Programming Interface", "--alias", "ap-i", "-p", "alpha"],
    );
    assert_eq!(code, 0, "term new failed: {stderr}");

    // ---- 12. mf term show "API" (B2.4 新命令) ----
    let (stdout, stderr, code) = run_in(ds.root(), &["term", "show", "API", "-p", "alpha"]);
    assert_eq!(code, 0, "term show failed: {stderr}");
    assert!(stdout.contains("API"), "term show should output term details: {stdout}");

    // ---- 13. mf project archive alpha (B1 archive) ----
    // Need git repo for archive to work
    std::process::Command::new("git").args(["init", "-q"]).current_dir(ds.root()).output().unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(ds.root())
        .output()
        .unwrap();
    std::process::Command::new("git").args(["config", "user.name", "Test"]).current_dir(ds.root()).output().unwrap();
    std::process::Command::new("git").args(["add", "-A"]).current_dir(ds.root()).output().unwrap();
    std::process::Command::new("git").args(["commit", "-m", "initial"]).current_dir(ds.root()).output().unwrap();

    let (stdout, stderr, code) = run_in(ds.root(), &["project", "archive", "alpha"]);
    assert_eq!(code, 0, "project archive failed: {stderr}");
    assert!(stdout.contains("Archived"), "archive should confirm: {stdout}");
}
