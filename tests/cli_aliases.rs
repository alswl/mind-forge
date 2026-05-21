use assert_cmd::Command;

mod common;

fn run(args: &[&str]) -> (String, String, i32) {
    let output = Command::cargo_bin("mf").expect("binary exists").args(args).output().expect("command runs");
    (
        String::from_utf8(output.stdout).expect("stdout utf8"),
        String::from_utf8(output.stderr).expect("stderr utf8"),
        output.status.code().unwrap_or_default(),
    )
}

/// T042: `mf terms list -p foo` ≡ `mf term list -p foo` (stderr clean)
#[test]
fn terms_alias_equals_term() {
    let dir = common::setup_repo();
    common::create_project(&dir, "test-proj");
    let (stdout_mind, stderr_mind, code_mind) =
        run(&["--root", &dir.path().to_string_lossy(), "terms", "list", "-p", "test-proj"]);
    assert_eq!(code_mind, 0, "terms alias should succeed");
    assert!(stderr_mind.is_empty(), "terms alias should produce no stderr warnings, got: {stderr_mind:?}");

    let (stdout_mf, stderr_mf, code_mf) =
        run(&["--root", &dir.path().to_string_lossy(), "term", "list", "-p", "test-proj"]);
    assert_eq!(code_mf, 0, "term primary should succeed");
    assert!(stderr_mf.is_empty(), "term primary should produce no stderr, got: {stderr_mf:?}");

    assert_eq!(stdout_mind, stdout_mf, "terms and term output should match");
}

/// T043: `mf project ls` ≡ `mf project list` (stderr clean)
#[test]
fn project_ls_alias() {
    let dir = common::setup_repo();
    let (stdout_ls, stderr_ls, code_ls) = run(&["--root", &dir.path().to_string_lossy(), "project", "ls"]);
    assert_eq!(code_ls, 0, "project ls should succeed");
    assert!(stderr_ls.is_empty(), "project ls should have clean stderr, got: {stderr_ls:?}");

    let (stdout_list, stderr_list, code_list) = run(&["--root", &dir.path().to_string_lossy(), "project", "list"]);
    assert_eq!(code_list, 0, "project list should succeed");
    assert!(stderr_list.is_empty(), "project list should have clean stderr");

    assert_eq!(stdout_ls, stdout_list, "ls and list output should match");
}

/// T044: `mf --json source list` ≡ `--format json source list`
#[test]
fn json_flag_equals_format_json() {
    let dir = common::setup_repo();
    common::create_project(&dir, "test-proj");
    let (stdout_json_flag, stderr_json_flag, code_json_flag) =
        run(&["--root", &dir.path().to_string_lossy(), "--json", "source", "list", "-p", "test-proj"]);
    assert_eq!(code_json_flag, 0, "--json flag should succeed");
    assert!(stderr_json_flag.is_empty(), "--json flag should produce no stderr warnings, got: {stderr_json_flag:?}");

    let (stdout_format, stderr_format, code_format) =
        run(&["--root", &dir.path().to_string_lossy(), "--format", "json", "source", "list", "-p", "test-proj"]);
    assert_eq!(code_format, 0, "--format json should succeed");
    assert!(stderr_format.is_empty(), "--format json should have clean stderr");

    assert_eq!(stdout_json_flag, stdout_format, "--json and --format json output should match");
}

/// T045: `mf --install-completion bash` equals `mf completion bash` (with deprecation on stderr)
#[test]
fn install_completion_flag() {
    let (stdout_install, stderr_install, code_install) = run(&["--install-completion", "bash"]);
    assert_eq!(code_install, 0);
    assert!(stdout_install.contains("_mf"), "completion output should contain 'mf' completion");
    assert!(
        stderr_install.contains("[deprecated]"),
        "--install-completion should emit deprecation warning, got: {stderr_install:?}"
    );
    assert!(
        stderr_install.contains("--install-completion"),
        "deprecation should mention --install-completion, got: {stderr_install:?}"
    );

    let (stdout_show, stderr_show, code_show) = run(&["--show-completion", "bash"]);
    assert_eq!(code_show, 0);
    assert!(stdout_show.contains("_mf"), "show-completion output should contain 'mf' completion");
    assert!(
        stderr_show.contains("[deprecated]"),
        "--show-completion should emit deprecation warning, got: {stderr_show:?}"
    );

    // Both --install-completion and --show-completion produce identical output
    assert_eq!(stdout_install, stdout_show, "--install-completion and --show-completion output should match");
}

/// T046: `mf version` ≡ `mf --version`
#[test]
fn version_subcommand_equals_version_flag() {
    let (stdout_sub, stderr_sub, code_sub) = run(&["version"]);
    assert_eq!(code_sub, 0, "mf version should succeed");
    assert!(stderr_sub.is_empty(), "mf version should have clean stderr, got: {stderr_sub:?}");

    let (stdout_flag, _stderr_flag, code_flag) = run(&["--version"]);
    assert_eq!(code_flag, 0, "mf --version should succeed");

    // Both should contain the version string
    assert!(stdout_sub.contains("mf "), "mf version output should contain 'mf ', got: {stdout_sub:?}");
    assert!(stdout_flag.contains("mf "), "mf --version output should contain 'mf ', got: {stdout_flag:?}");
}

/// T048: `mf --json --format text` -> `--json` wins (no error)
#[test]
fn json_flag_wins_over_format() {
    let dir = common::setup_repo();
    common::create_project(&dir, "test-proj");
    // When both --json and --format text are specified, --json should take precedence
    let (_stdout, stderr, code) = run(&[
        "--root",
        &dir.path().to_string_lossy(),
        "--json",
        "--format",
        "text",
        "source",
        "list",
        "-p",
        "test-proj",
    ]);
    assert_eq!(code, 0, "should succeed even with both --json and --format");
    assert!(
        stderr.is_empty() || stderr.contains("[deprecated]"),
        "stderr should be clean or contain only deprecation warnings, got: {stderr:?}"
    );
}

/// T049: `mf terms list` (double alias: terms + no subcommand alias) still clean stderr
/// Note: `ls` is not an alias on `term list`, only on article/asset/project/source list.
/// The "double alias" for `term` uses `terms` (top-level alias) + `list` (primary).
#[test]
fn double_alias_terms_list_clean_stderr() {
    let dir = common::setup_repo();
    common::create_project(&dir, "test-proj");
    let (_stdout, stderr, code) = run(&["--root", &dir.path().to_string_lossy(), "terms", "list", "-p", "test-proj"]);
    assert_eq!(code, 0, "terms list should succeed");
    assert!(stderr.is_empty(), "double alias 'terms list' should produce no warnings, got: {stderr:?}");
}

/// T047: short flag -p for --project works
#[test]
fn short_flag_project() {
    let dir = common::setup_repo();
    common::create_project(&dir, "test-proj");
    let (_stdout_short, stderr_short, code_short) =
        run(&["--root", &dir.path().to_string_lossy(), "source", "list", "-p", "test-proj"]);
    assert_eq!(code_short, 0);
    assert!(stderr_short.is_empty() || stderr_short.contains("[deprecated]"));
}

/// T047: short flag -t for --type in source add
#[test]
fn short_flag_type() {
    // --type is only used in source add/list, but source add needs actual files
    // Just verify the flag parses correctly
    let dir = common::setup_repo();
    common::create_project(&dir, "test-proj");
    // List with -t should be equivalent to --type
    let (_stdout_short, stderr_short, code_short) =
        run(&["--root", &dir.path().to_string_lossy(), "source", "list", "-t", "pdf", "-p", "test-proj"]);
    assert_eq!(code_short, 0);
    assert!(
        stderr_short.is_empty() || stderr_short.contains("[deprecated]"),
        "stderr should be clean or contain only expected deprecation warnings, got: {stderr_short:?}"
    );
}

/// T047: short flag -f for --force in source add (parse only, no actual file)
#[test]
fn short_flag_force() {
    // Just verify -f parses in --help context
    let (stdout, _, code) = run(&["source", "add", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("-f"), "source add --help should show -f short flag");
}

/// T047: short flag -o for --output in build
#[test]
fn short_flag_output() {
    let (stdout, _, code) = run(&["build", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("-o"), "build --help should show -o short flag");
}

/// T047: short flag -n for --name in source add
#[test]
fn short_flag_name() {
    let (stdout, _, code) = run(&["source", "add", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("-n"), "source add --help should show -n short flag");
}

/// Verify `mf project info` alias is removed (neither visible nor hidden)
#[test]
fn project_info_alias_removed() {
    let dir = common::setup_repo();
    common::create_project(&dir, "test-proj");
    let (_stdout, stderr, code) = run(&["--root", &dir.path().to_string_lossy(), "project", "info", "-p", "test-proj"]);
    assert_ne!(code, 0, "project info should fail (alias removed)");
    assert!(
        stderr.contains("unrecognized subcommand 'info'"),
        "should get unrecognized subcommand error, got: {stderr:?}"
    );
}

/// Verify `mf project status` is a hidden compatibility alias for `project show`
#[test]
fn project_status_deprecated_alias() {
    let dir = common::setup_repo();
    common::create_project(&dir, "test-proj");
    let (_stdout, stderr, code) =
        run(&["--root", &dir.path().to_string_lossy(), "project", "status", "-p", "test-proj"]);
    assert_eq!(code, 0, "project status should still succeed");
    assert!(stderr.contains("[deprecated]"), "project status should emit deprecation warning, got: {stderr:?}");
    assert!(stderr.contains("project show"), "deprecation should mention project show, got: {stderr:?}");
}

/// Verify `mf asset ls` alias works
#[test]
fn asset_ls_alias() {
    let dir = common::setup_repo();
    common::create_project(&dir, "test-proj");
    let (stdout_ls, stderr_ls, code_ls) =
        run(&["--root", &dir.path().to_string_lossy(), "asset", "ls", "-p", "test-proj"]);
    assert_eq!(code_ls, 0, "asset ls should succeed");
    assert!(stderr_ls.is_empty(), "asset ls should have clean stderr, got: {stderr_ls:?}");

    let (stdout_list, _stderr_list, code_list) =
        run(&["--root", &dir.path().to_string_lossy(), "asset", "list", "-p", "test-proj"]);
    assert_eq!(code_list, 0);
    assert_eq!(stdout_ls, stdout_list, "asset ls and list output should match");
}

/// Verify `mf article ls` alias works
#[test]
fn article_ls_alias() {
    let dir = common::setup_repo();
    common::create_project(&dir, "test-proj");
    let (stdout_ls, stderr_ls, code_ls) =
        run(&["--root", &dir.path().to_string_lossy(), "article", "ls", "-p", "test-proj"]);
    assert_eq!(code_ls, 0, "article ls should succeed");
    assert!(stderr_ls.is_empty(), "article ls should have clean stderr, got: {stderr_ls:?}");

    let (stdout_list, _stderr_list, code_list) =
        run(&["--root", &dir.path().to_string_lossy(), "article", "list", "-p", "test-proj"]);
    assert_eq!(code_list, 0);
    assert_eq!(stdout_ls, stdout_list, "article ls and list output should match");
}

// ── T012: Verb consistency across resource families ──────────────────────

/// Verify `new` verb is used for local entity creation across resource groups
#[test]
fn verb_new_for_local_creation() {
    let (stdout_p, _, code_p) = run(&["project", "--help"]);
    assert_eq!(code_p, 0);
    assert!(stdout_p.contains("new"), "project should have 'new' subcommand");

    let (stdout_a, _, code_a) = run(&["article", "--help"]);
    assert_eq!(code_a, 0);
    assert!(stdout_a.contains("new"), "article should have 'new' subcommand");

    let (stdout_t, _, code_t) = run(&["term", "--help"]);
    assert_eq!(code_t, 0);
    assert!(stdout_t.contains("new"), "term should have 'new' subcommand");
}

/// Verify `add` verb is used for external registration across resource groups
#[test]
fn verb_add_for_external_registration() {
    let (stdout_s, _, code_s) = run(&["source", "--help"]);
    assert_eq!(code_s, 0);
    assert!(stdout_s.contains("add"), "source should have 'add' subcommand");

    let (stdout_a, _, code_a) = run(&["asset", "--help"]);
    assert_eq!(code_a, 0);
    assert!(stdout_a.contains("add"), "asset should have 'add' subcommand");

    // term add (renamed from learn) should exist
    let (stdout_t, _, code_t) = run(&["term", "--help"]);
    assert_eq!(code_t, 0);
    assert!(stdout_t.contains("add"), "term should have 'add' subcommand (renamed from learn)");
}

/// Verify `list` verb exists on all resource groups
#[test]
fn verb_list_on_all_resources() {
    for resource in &["source", "asset", "project", "article", "term"] {
        let (stdout, _, code) = run(&[resource, "--help"]);
        assert_eq!(code, 0, "{resource} --help should succeed");
        assert!(stdout.contains("list"), "{resource} should have 'list' subcommand");
    }
}

/// Verify `show` verb exists on all resource groups (target state)
#[test]
fn verb_show_on_all_resources() {
    for resource in &["source", "asset", "project", "article", "term"] {
        let (stdout, _, code) = run(&[resource, "--help"]);
        assert_eq!(code, 0, "{resource} --help should succeed");
        assert!(stdout.contains("show"), "{resource} should have 'show' subcommand");
    }
}

/// Verify term-specific: `update` replaces `fix`, `add` replaces `learn`
#[test]
fn term_renamed_verbs() {
    let (stdout, _, code) = run(&["term", "--help"]);
    assert_eq!(code, 0);

    // Target state: should have 'update' and 'add'
    assert!(stdout.contains("update"), "term should have 'update' subcommand (renamed from fix)");
    assert!(stdout.contains("add"), "term should have 'add' subcommand (renamed from learn)");

    // Old names should NOT appear in help (they become hidden aliases)
    assert!(!stdout.contains("learn"), "term help should not show 'learn' (hidden alias)");
    assert!(!stdout.contains("fix"), "term help should not show 'fix' (hidden alias)");
}

/// Verify `mf source ls` alias works
#[test]
fn source_ls_alias() {
    let dir = common::setup_repo();
    common::create_project(&dir, "test-proj");
    let (stdout_ls, stderr_ls, code_ls) =
        run(&["--root", &dir.path().to_string_lossy(), "source", "ls", "-p", "test-proj"]);
    assert_eq!(code_ls, 0, "source ls should succeed");
    assert!(stderr_ls.is_empty(), "source ls should have clean stderr, got: {stderr_ls:?}");

    let (stdout_list, _stderr_list, code_list) =
        run(&["--root", &dir.path().to_string_lossy(), "source", "list", "-p", "test-proj"]);
    assert_eq!(code_list, 0);
    assert_eq!(stdout_ls, stdout_list, "source ls and list output should match");
}

// ── rm alias regression tests (T021) ───────────────────────────────────────

/// Verify `rm` alias is visible and recognized on all resource groups
#[test]
fn rm_alias_on_all_resources() {
    for resource in &["source", "asset", "project", "article", "term"] {
        let (stdout, _, code) = run(&[resource, "--help"]);
        assert_eq!(code, 0, "{resource} --help should succeed");
        assert!(stdout.contains("remove"), "{resource} should have 'remove' subcommand");
        // The rm alias should be visible in clap output (shown as visible_alias)
    }
}

/// Verify `term rm` is recognized (not "unrecognized subcommand")
#[test]
fn term_rm_recognized() {
    let dir = common::setup_repo();
    common::create_project(&dir, "test-proj");
    let (_stdout, stderr, code) = run(&["--root", &dir.path().to_string_lossy(), "term", "rm", "--help"]);
    assert_eq!(code, 0, "term rm --help should succeed");
    assert!(!stderr.contains("unrecognized"), "term rm should be recognized, got: {stderr:?}");
}

/// Verify `source rm` is recognized
#[test]
fn source_rm_recognized() {
    let dir = common::setup_repo();
    common::create_project(&dir, "test-proj");
    let (_stdout, stderr, code) = run(&["--root", &dir.path().to_string_lossy(), "source", "rm", "--help"]);
    assert_eq!(code, 0, "source rm --help should succeed");
    assert!(!stderr.contains("unrecognized"), "source rm should be recognized, got: {stderr:?}");
}

/// Verify `asset rm` is recognized
#[test]
fn asset_rm_recognized() {
    let dir = common::setup_repo();
    common::create_project(&dir, "test-proj");
    let (_stdout, stderr, code) = run(&["--root", &dir.path().to_string_lossy(), "asset", "rm", "--help"]);
    assert_eq!(code, 0, "asset rm --help should succeed");
    assert!(!stderr.contains("unrecognized"), "asset rm should be recognized, got: {stderr:?}");
}

/// Verify `article rm` is recognized
#[test]
fn article_rm_recognized() {
    let dir = common::setup_repo();
    common::create_project(&dir, "test-proj");
    let (_stdout, stderr, code) = run(&["--root", &dir.path().to_string_lossy(), "article", "rm", "--help"]);
    assert_eq!(code, 0, "article rm --help should succeed");
    assert!(!stderr.contains("unrecognized"), "article rm should be recognized, got: {stderr:?}");
}

/// Verify `project rm` is recognized
#[test]
fn project_rm_recognized() {
    let dir = common::setup_repo();
    common::create_project(&dir, "test-proj");
    let (_stdout, stderr, code) = run(&["--root", &dir.path().to_string_lossy(), "project", "rm", "--help"]);
    assert_eq!(code, 0, "project rm --help should succeed");
    assert!(!stderr.contains("unrecognized"), "project rm should be recognized, got: {stderr:?}");
}

/// Verify `term ls` alias is recognized
#[test]
fn term_ls_alias() {
    let dir = common::setup_repo();
    common::create_project(&dir, "test-proj");
    let (stdout_ls, stderr_ls, code_ls) =
        run(&["--root", &dir.path().to_string_lossy(), "term", "ls", "-p", "test-proj"]);
    assert_eq!(code_ls, 0, "term ls should succeed");
    assert!(stderr_ls.is_empty(), "term ls should have clean stderr, got: {stderr_ls:?}");

    let (stdout_list, _stderr_list, code_list) =
        run(&["--root", &dir.path().to_string_lossy(), "term", "list", "-p", "test-proj"]);
    assert_eq!(code_list, 0);
    assert_eq!(stdout_ls, stdout_list, "term ls and list output should match");
}
