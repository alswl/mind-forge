use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;

fn run(args: &[&str]) -> (String, String, i32) {
    let output = Command::cargo_bin("mf").expect("binary exists").args(args).output().expect("command runs");
    (
        String::from_utf8(output.stdout).expect("stdout utf8"),
        String::from_utf8(output.stderr).expect("stderr utf8"),
        output.status.code().unwrap_or_default(),
    )
}

fn run_in_dir(args: &[&str], cwd: &TempDir) -> (String, String, i32) {
    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .current_dir(cwd.path())
        .args(args)
        .output()
        .expect("command runs");
    (
        String::from_utf8(output.stdout).expect("stdout utf8"),
        String::from_utf8(output.stderr).expect("stderr utf8"),
        output.status.code().unwrap_or_default(),
    )
}

// ── US9: Rich text output ────────────────────────────────────────────────

#[test]
fn version_text_output() {
    let (stdout, stderr, code) = run(&["version"]);
    assert_eq!(code, 0, "stderr: {stderr:?}");
    // Format: mf X.Y.Z (commit, built YYYY-MM-DD, rustc VERSION)
    assert!(stdout.starts_with("mf "), "expected 'mf <version>', got: {stdout:?}");
    assert!(stdout.contains('('), "should contain opening paren with metadata: {stdout:?}");
    assert!(stdout.contains(')'), "should contain closing paren: {stdout:?}");
    assert!(stdout.contains("built"), "should contain 'built': {stdout:?}");
    assert!(stdout.contains("rustc"), "should contain 'rustc': {stdout:?}");
    assert!(stdout.trim().len() > 3, "version should be non-empty after 'mf '");
    assert!(stderr.is_empty(), "stderr should be empty on success, got: {stderr:?}");
}

// ── US1: JSON output ────────────────────────────────────────────────────

#[test]
fn version_json_output_via_json_flag() {
    let (stdout, stderr, code) = run(&["--json", "version"]);
    assert_eq!(code, 0, "stderr: {stderr:?}");
    assert!(stderr.is_empty(), "stderr should be empty on success");
    let v: Value = serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert_eq!(v["status"], "ok");
    assert_eq!(v["command"], "mf");
    let version = v["data"]["version"].as_str().expect("data.version should be a string");
    assert!(!version.is_empty(), "data.version should be non-empty");
}

#[test]
fn version_json_output_via_format_json() {
    let (stdout, stderr, code) = run(&["--output", "json", "version"]);
    assert_eq!(code, 0, "stderr: {stderr:?}");
    assert!(stderr.is_empty(), "stderr should be empty on success");
    let v: Value = serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert_eq!(v["status"], "ok");
    assert!(v["data"]["version"].as_str().is_some_and(|s| !s.is_empty()));
    // Rich fields (T105)
    assert!(v["data"]["commit"].is_string(), "commit should be present: {stdout:?}");
    assert!(v["data"]["build_date"].is_string(), "build_date should be present: {stdout:?}");
    assert!(v["data"]["rustc"].is_string(), "rustc should be present: {stdout:?}");
    assert!(v["data"]["target_triple"].is_string(), "target_triple should be present: {stdout:?}");
    let target = v["data"]["target_triple"].as_str().unwrap();
    assert!(target.contains('-'), "target_triple should contain dash separator: {target:?}");
}

// ── US1: Non-repo cwd ───────────────────────────────────────────────────

#[test]
fn version_works_outside_repo() {
    let dir = TempDir::new().unwrap();
    let (stdout, stderr, code) = run_in_dir(&["version"], &dir);
    assert_eq!(code, 0, "should succeed outside repo, stderr: {stderr:?}");
    assert!(stdout.starts_with("mf "));
    assert!(stdout.contains("built"), "outside repo output should still have rich info: {stdout:?}");
}

#[test]
fn version_json_works_outside_repo() {
    let dir = TempDir::new().unwrap();
    let (stdout, stderr, code) = run_in_dir(&["--json", "version"], &dir);
    assert_eq!(code, 0, "should succeed outside repo, stderr: {stderr:?}");
    let v: Value = serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert_eq!(v["status"], "ok");
}

// ── US1: Read-only side-effect ──────────────────────────────────────────

#[test]
fn version_is_read_only() {
    let dir = TempDir::new().unwrap();
    let initial: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
    run_in_dir(&["version"], &dir);
    let after: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
    assert_eq!(initial.len(), after.len(), "no files should be created by 'mf version'");
}

#[test]
fn version_json_is_read_only() {
    let dir = TempDir::new().unwrap();
    let initial: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
    run_in_dir(&["--json", "version"], &dir);
    let after: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
    assert_eq!(initial.len(), after.len(), "no files should be created by 'mf --json version'");
}

// ── US1: Regression: mf --version ───────────────────────────────────────

#[test]
fn dash_dash_version_still_works() {
    // clap's built-in --version with propagate_version
    let output = Command::cargo_bin("mf").expect("binary exists").arg("--version").output().expect("command runs");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("mf"), "'mf --version' should contain name: {stdout:?}");
}
