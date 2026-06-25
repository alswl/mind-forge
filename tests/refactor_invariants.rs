//! Structural guard tests that scan source files to assert invariants after
//! the Rust CLI Guide alignment refactor. These tests encode SC-002.

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// SC-002: No `std::env::current_dir()` in `src/cli/`.
#[test]
fn no_global_current_dir_in_cli_layer() {
    let cli_dir = repo_root().join("src").join("cli");
    let entries = fs::read_dir(&cli_dir).expect("src/cli/ exists");
    let mut found = vec![];
    for entry in entries {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "rs") {
            let content = fs::read_to_string(&path).unwrap();
            if content.contains("std::env::current_dir") {
                found.push(path.file_name().unwrap().to_string_lossy().to_string());
            }
        }
    }
    if !found.is_empty() {
        // Display full paths for debugging
        let paths = found.iter().map(|f| format!("src/cli/{f}")).collect::<Vec<_>>().join(", ");
        panic!(
            "SC-002 violation: found std::env::current_dir() in CLI layer: {paths}. \
             Use ctx.cwd() from CommandCtx instead."
        );
    }
}

/// SC-002: No `set_var("NO_COLOR"` in `src/main.rs`.
#[test]
fn no_set_var_no_color_in_main() {
    let main_rs = repo_root().join("src").join("main.rs");
    let content = fs::read_to_string(&main_rs).unwrap();
    assert!(
        !content.contains("set_var(\"NO_COLOR\""),
        "SC-002 violation: found set_var(\"NO_COLOR\", ...) in src/main.rs. \
         Thread color preference through CommandCtx + render() instead."
    );
}
