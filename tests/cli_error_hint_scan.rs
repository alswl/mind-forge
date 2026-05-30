use assert_cmd::Command;
use std::collections::HashSet;
use std::fs;

mod common;

/// Collect source files in src/ (production code only, not test files).
fn source_files() -> Vec<String> {
    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new("src").into_iter().flatten() {
        let path = entry.path();
        if path.extension().map(|e| e == "rs").unwrap_or(false) {
            files.push(path.to_string_lossy().to_string());
        }
    }
    files
}

/// Return true if a line is a comment (starts with // or is inside /* */).
fn is_comment(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*')
}

/// Extract all backtick-quoted `mf <token>` references from source content.
fn extract_mf_commands(content: &str) -> Vec<String> {
    let mut cmds = Vec::new();
    for line in content.lines() {
        if is_comment(line) {
            continue;
        }
        let mut start = 0;
        while let Some(pos) = line[start..].find("`mf ") {
            let abs = start + pos + 1; // after the opening backtick
            let cmd_start = abs + 3; // skip "mf "
            if let Some(end) = line[cmd_start..].find('`') {
                let full = &line[cmd_start..cmd_start + end];
                let subcmd = full.split_whitespace().next().unwrap_or(full);
                cmds.push(subcmd.to_string());
            }
            start = abs + 1;
        }
    }
    cmds
}

/// Parse `mf --help` to get all top-level subcommands.
fn top_level_subcommands() -> HashSet<String> {
    let output = Command::cargo_bin("mf").expect("binary exists").arg("--help").output().expect("help runs");

    let stdout = String::from_utf8(output.stdout).expect("utf8");
    let mut subcmds = HashSet::new();
    let mut in_commands = false;

    for line in stdout.lines() {
        if line.trim() == "Commands:" {
            in_commands = true;
            continue;
        }
        if in_commands {
            let trimmed = line.trim();
            if trimmed.is_empty() || !trimmed.starts_with(|c: char| c.is_alphabetic()) {
                break;
            }
            if let Some(cmd) = trimmed.split_whitespace().next() {
                subcmds.insert(cmd.to_string());
            }
        }
    }
    subcmds
}

// =============================================================================
// T109: Error hint conventions — backticks, not single quotes
// =============================================================================

#[test]
fn no_single_quoted_mf_commands_in_hints() {
    for file in source_files() {
        let content = fs::read_to_string(&file).unwrap_or_default();
        for (i, line) in content.lines().enumerate() {
            if is_comment(line) {
                continue;
            }
            if line.contains("'mf ") {
                panic!("{file}:{} has single-quoted mf command: {line}", i + 1);
            }
        }
    }
}

// =============================================================================
// T109: Error hint conventions — all referenced commands exist
// =============================================================================

#[test]
fn all_mf_commands_in_hints_exist() {
    let subcmds = top_level_subcommands();
    assert!(!subcmds.is_empty(), "should find subcommands in --help");

    // Known valid items that might appear after `mf ` in backticks
    let known_valid: HashSet<&str> = [
        // Top-level subcommands
        "article",
        "asset",
        "source",
        "term",
        "project",
        "publish",
        "render",
        "config",
        // Nested subcommands
        "list",
        "show",
        "new",
        "add",
        "remove",
        "rename",
        "update",
        "index",
        "lint",
        "fix",
        "learn",
        "upgrade",
        "init",
        // Global flags
        "--help",
        "--json",
        "--format",
        "--no-headers",
        "--no-trunc",
        "--version",
        "-p",
    ]
    .iter()
    .copied()
    .collect();

    for file in source_files() {
        let content = fs::read_to_string(&file).unwrap_or_default();
        let cmds = extract_mf_commands(&content);
        for cmd in &cmds {
            // Allow <placeholder> tokens used in examples
            if cmd.starts_with('<') || cmd.starts_with("--") || cmd == "-p" {
                continue;
            }
            if known_valid.contains(cmd.as_str()) || subcmds.contains(cmd.as_str()) {
                continue;
            }
            panic!("unknown mf subcommand '{cmd}' referenced in hint in {file}");
        }
    }
}

// =============================================================================
// T110: Deprecated hint check
// =============================================================================

#[test]
fn no_deprecated_config_init_hint() {
    for file in source_files() {
        let content = fs::read_to_string(&file).unwrap_or_default();
        assert!(
            !content.contains("config init --target"),
            "{file} should not reference deprecated `mf config init --target project`"
        );
    }
}

// =============================================================================
// T110 (extended): Verify the correct init hint is present
// =============================================================================

#[test]
fn init_hint_uses_backtick_mf_init() {
    // the error.rs INIT_REPO_HINT must use backticks
    let content = fs::read_to_string("src/error.rs").expect("can read error.rs");
    assert!(content.contains("`mf init`"), "INIT_REPO_HINT should use backtick-quoted `mf init`");
    assert!(
        !content.contains("'mf config init --target project'"),
        "INIT_REPO_HINT should NOT contain deprecated config init command"
    );
}
