//! CLI integration tests for project creation by path identity (US1).
//! Covers T016-T020.

use assert_cmd::Command;
use tempfile::TempDir;

mod common;

fn setup_flat_repo() -> TempDir {
    common::setup_repo()
}

/// A non-flat repo (`projects_dir: projects`) with one project containing
/// one article — the shape the reported #55 bug requires: `--project .`
/// only diverges from the bare form when the projects directory is a real,
/// distinct location that a bad fallback could land on.
fn setup_nonflat_repo_with_project() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("minds.yaml"), "schema: '1'\nprojects_dir: projects\nprojects:\n- projects/projA\n")
        .unwrap();
    let project = dir.path().join("projects/projA");
    std::fs::create_dir_all(project.join("docs/x")).unwrap();
    std::fs::create_dir_all(project.join("sources")).unwrap();
    std::fs::create_dir_all(project.join("assets")).unwrap();
    std::fs::write(project.join("mind.yaml"), "schema_version: '1'\n").unwrap();
    std::fs::write(project.join("docs/x/01-opening.md"), "# Opening\n").unwrap();
    (dir, project)
}

fn mf(args: &[&str], repo: &TempDir) -> (String, String, i32) {
    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap()])
        .args(args)
        .current_dir(repo.path())
        .output()
        .unwrap();
    (
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
        output.status.code().unwrap_or_default(),
    )
}

fn mf_json(args: &[&str], repo: &TempDir) -> (String, String, i32) {
    let mut full_args = vec!["--root", repo.path().to_str().unwrap(), "--json"];
    full_args.extend_from_slice(args);
    let output = Command::cargo_bin("mf").unwrap().args(&full_args).current_dir(repo.path()).output().unwrap();
    (
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
        output.status.code().unwrap_or_default(),
    )
}

/// Like `mf`/`mf_json`, but runs with an explicit cwd instead of the repo
/// root — needed for every `--project .`-shaped test, since the whole bug is
/// about resolution relative to where the user actually stands.
fn mf_at(args: &[&str], repo_root: &std::path::Path, cwd: &std::path::Path, json: bool) -> (String, String, i32) {
    let mut full_args = vec!["--root", repo_root.to_str().unwrap()];
    if json {
        full_args.push("--json");
    }
    full_args.extend_from_slice(args);
    let output = Command::cargo_bin("mf").unwrap().args(&full_args).current_dir(cwd).output().unwrap();
    (
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
        output.status.code().unwrap_or_default(),
    )
}

// ── T016: Create project with Unicode path from repo root ───────────────

#[test]
fn project_new_unicode_path_from_repo_root() {
    let repo = setup_flat_repo();
    let (stdout, stderr, code) =
        mf_json(&["project", "new", "workspaces/E_团队周报/projects/2026-W21_iSee团队周报"], &repo);
    assert_eq!(code, 0, "stderr: {stderr}");

    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    assert_eq!(v["data"]["path"], "workspaces/E_团队周报/projects/2026-W21_iSee团队周报");
    assert_eq!(v["data"]["details"]["requested_path"], "workspaces/E_团队周报/projects/2026-W21_iSee团队周报");

    // Verify directory was created
    let project_dir = repo.path().join("workspaces/E_团队周报/projects/2026-W21_iSee团队周报");
    assert!(project_dir.join("mind.yaml").exists());
}

// ── T017: Create project cwd-relative ────────────────────────────────────

#[test]
fn project_new_cwd_relative() {
    let repo = setup_flat_repo();
    let ws_dir = repo.path().join("workspaces/E_团队周报/projects");
    std::fs::create_dir_all(&ws_dir).unwrap();

    let output = Command::cargo_bin("mf")
        .unwrap()
        .args(["--root", repo.path().to_str().unwrap(), "--json", "project", "new", "2026-W21_iSee团队周报"])
        .current_dir(&ws_dir)
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(output.status.success(), "stderr: {stderr}");

    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    assert_eq!(v["data"]["path"], "workspaces/E_团队周报/projects/2026-W21_iSee团队周报");
    assert_eq!(v["data"]["details"]["requested_path"], "2026-W21_iSee团队周报");
}

// ── T018: Create project with emoji path ─────────────────────────────────

#[test]
fn project_new_emoji_path() {
    let repo = setup_flat_repo();
    let (stdout, stderr, code) = mf_json(&["project", "new", "workspaces/📊周报/projects/2026-W21_团队复盘🚀"], &repo);
    assert_eq!(code, 0, "stderr: {stderr}");

    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    assert!(v["data"]["path"].as_str().unwrap().contains("📊周报"));
    assert!(v["data"]["path"].as_str().unwrap().contains("团队复盘🚀"));

    let project_dir = repo.path().join("workspaces/📊周报/projects/2026-W21_团队复盘🚀");
    assert!(project_dir.join("mind.yaml").exists());
}

// ── T019: JSON envelope shape for project new success ────────────────────

#[test]
fn project_new_json_envelope() {
    let repo = setup_flat_repo();
    let (stdout, stderr, code) = mf_json(&["project", "new", "alpha"], &repo);
    assert_eq!(code, 0, "stderr: {stderr}");

    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    assert_eq!(v["command"], "mf");

    let data = &v["data"];
    assert!(data["details"]["path"].is_string(), "data.details.path missing: {data}");
    assert!(data["details"]["requested_path"].is_string(), "data.details.requested_path missing: {data}");
    assert!(data["details"]["created_at"].is_string(), "data.details.created_at missing: {data}");
    assert!(data["details"]["scaffolded"].is_array(), "data.details.scaffolded missing or not array: {data}");
}

// ── T020: Error cases ────────────────────────────────────────────────────

#[test]
fn project_new_rejects_path_escape() {
    let repo = setup_flat_repo();
    let (stdout, stderr, code) = mf_json(&["project", "new", "../outside"], &repo);
    assert_ne!(code, 0, "should fail: stdout={stdout} stderr={stderr}");

    // Error payload goes to stderr in JSON mode
    let error_output = if stderr.trim().is_empty() { stdout } else { stderr };
    let v: serde_json::Value = serde_json::from_str(&error_output).unwrap();
    assert_eq!(v["status"], "error");
    let err_msg = v["error"]["message"].as_str().unwrap();
    assert!(
        err_msg.contains("..") || err_msg.contains("outside") || err_msg.contains("escape"),
        "unexpected error message: {err_msg}"
    );
}

#[test]
fn project_new_rejects_duplicate_path() {
    let repo = setup_flat_repo();
    mf(&["project", "new", "dupe"], &repo);

    let (stdout, stderr, code) = mf_json(&["project", "new", "dupe"], &repo);
    assert_ne!(code, 0, "duplicate project should fail: stdout={stdout} stderr={stderr}");

    let error_output = if stderr.trim().is_empty() { stdout } else { stderr };
    let v: serde_json::Value = serde_json::from_str(&error_output).unwrap();
    assert_eq!(v["status"], "error");
}

#[test]
fn project_new_rejects_duplicate_nested_path() {
    let repo = setup_flat_repo();
    mf(&["project", "new", "workspaces/team/projects/my-report"], &repo);

    let (stdout, stderr, code) = mf_json(&["project", "new", "workspaces/team/projects/my-report"], &repo);
    assert_ne!(code, 0, "duplicate nested project should fail: stdout={stdout} stderr={stderr}");

    let error_output = if stderr.trim().is_empty() { stdout } else { stderr };
    let v: serde_json::Value = serde_json::from_str(&error_output).unwrap();
    assert_eq!(v["status"], "error");
}

#[test]
fn project_new_simple_name_still_works() {
    let repo = setup_flat_repo();
    let (stdout, stderr, code) = mf_json(&["project", "new", "simple-report"], &repo);
    assert_eq!(code, 0, "stderr: {stderr}");

    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["status"], "ok");
    assert_eq!(v["data"]["path"], "simple-report");

    assert!(repo.path().join("simple-report/mind.yaml").exists());
}

#[test]
fn project_dot_matches_bare_form_for_article_index() {
    let (dir, project) = setup_nonflat_repo_with_project();
    let (stdout_dot, stderr_dot, code_dot) = mf_at(&["article", "index", "--project", "."], dir.path(), &project, true);
    assert_eq!(code_dot, 0, "stderr: {stderr_dot}");
    let dot: serde_json::Value = serde_json::from_str(&stdout_dot).unwrap();

    let (stdout_bare, stderr_bare, code_bare) = mf_at(&["article", "index"], dir.path(), &project, true);
    assert_eq!(code_bare, 0, "stderr: {stderr_bare}");
    let bare: serde_json::Value = serde_json::from_str(&stdout_bare).unwrap();

    assert_eq!(
        dot["data"]["scanned_count"], bare["data"]["scanned_count"],
        "--project . diverged from the bare form: dot={dot} bare={bare}"
    );
    assert_eq!(dot["data"]["scanned_count"], 1, "expected the one fixture article to be scanned");
}

#[test]
fn project_dot_and_dot_slash_and_dot_slash_sub_all_resolve() {
    let (dir, project) = setup_nonflat_repo_with_project();
    for input in [".", "./"] {
        let (stdout, stderr, code) = mf_at(&["article", "index", "--project", input], dir.path(), &project, true);
        assert_eq!(code, 0, "input={input} stderr={stderr}");
        let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(v["data"]["scanned_count"], 1, "input={input}: {v}");
    }

    // `./sub` form: run from the project's parent, pointing at the project by
    // its relative subdirectory name.
    let parent = project.parent().unwrap().to_path_buf();
    let subdir_selector = format!("./{}", project.file_name().unwrap().to_str().unwrap());
    let (stdout, stderr, code) = mf_at(&["article", "index", "--project", &subdir_selector], dir.path(), &parent, true);
    assert_eq!(code, 0, "stderr: {stderr}");
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["data"]["scanned_count"], 1, "{v}");
}

#[test]
fn project_dot_matches_bare_form_for_project_lint() {
    let (dir, project) = setup_nonflat_repo_with_project();
    let (_, stderr_dot, code_dot) = mf_at(&["project", "lint", "--project", "."], dir.path(), &project, false);
    assert_eq!(code_dot, 0, "project lint --project . should succeed: stderr={stderr_dot}");

    let (_, stderr_bare, code_bare) = mf_at(&["project", "lint"], dir.path(), &project, false);
    assert_eq!(code_bare, 0, "stderr: {stderr_bare}");
    assert_eq!(code_dot, code_bare);
}

#[test]
fn project_selector_to_directory_without_manifest_is_refused_not_ok() {
    let (dir, project) = setup_nonflat_repo_with_project();
    // A directory that exists but is not a project (no mind.yaml).
    std::fs::create_dir_all(project.parent().unwrap().join("not-a-project")).unwrap();
    let (stdout, stderr, code) =
        mf_at(&["article", "index", "--project", "not-a-project"], dir.path(), project.parent().unwrap(), true);
    assert_ne!(code, 0, "must refuse, not report ok: stdout={stdout} stderr={stderr}");
    let error_output = if stderr.trim().is_empty() { stdout } else { stderr };
    let v: serde_json::Value = serde_json::from_str(&error_output).unwrap();
    assert_eq!(v["status"], "error");
    let msg = v["error"]["message"].as_str().unwrap_or_default();
    assert!(msg.contains("not-a-project") || msg.contains("not found"), "message should name the location: {msg}");
}

#[test]
fn project_dotdot_is_refused_with_accurate_message() {
    let (dir, project) = setup_nonflat_repo_with_project();
    let (stdout, stderr, code) = mf_at(&["article", "list", "--project", ".."], dir.path(), &project, true);
    assert_ne!(code, 0, "bare '..' must be refused: stdout={stdout} stderr={stderr}");
    let error_output = if stderr.trim().is_empty() { stdout } else { stderr };
    let v: serde_json::Value = serde_json::from_str(&error_output).unwrap();
    assert_eq!(v["status"], "error");
    let message = v["error"]["message"].as_str().unwrap_or_default();
    assert!(message.contains(".."), "message must identify the rejected traversal form: {message}");
    assert!(
        message.contains("not accepted"),
        "message must state the selector restriction rather than treating '..' as a missing name: {message}"
    );
}

#[test]
fn project_dotdot_sibling_is_refused_without_a_false_escape_claim() {
    let (dir, project) = setup_nonflat_repo_with_project();
    // projB is a *real sibling inside the repo* — the refusal must still
    // happen (strict `..` policy), but the message must not claim the path
    // "would escape the repo root" when it plainly would not.
    std::fs::create_dir_all(dir.path().join("projects/projB")).unwrap();
    std::fs::write(dir.path().join("projects/projB/mind.yaml"), "schema_version: '1'\n").unwrap();

    let (stdout, stderr, code) = mf_at(&["article", "list", "--project", "../projB"], dir.path(), &project, true);
    assert_ne!(code, 0, "'..' must be refused even for an in-repo sibling: stdout={stdout} stderr={stderr}");
    let error_output = if stderr.trim().is_empty() { stdout } else { stderr };
    let v: serde_json::Value = serde_json::from_str(&error_output).unwrap();
    assert_eq!(v["status"], "error");
    let msg = v["error"]["message"].as_str().unwrap_or_default();
    assert!(msg.contains(".."), "message should name the restriction: {msg}");
    assert!(
        !msg.to_lowercase().contains("would escape"),
        "message must not assert a false consequence for an in-repo sibling: {msg}"
    );
}

#[test]
fn project_bare_name_shadowed_by_cwd_subdir_is_refused_not_guessed() {
    let (dir, project) = setup_nonflat_repo_with_project();
    // A subdirectory of projA, named "B", that is itself a valid project —
    // and a *different*, also-registered top-level project also named "B".
    std::fs::create_dir_all(project.join("B/docs")).unwrap();
    std::fs::write(project.join("B/mind.yaml"), "schema_version: '1'\n").unwrap();
    std::fs::write(project.join("B/docs/child.md"), "# Child\n").unwrap();

    std::fs::create_dir_all(dir.path().join("projects/B")).unwrap();
    std::fs::write(dir.path().join("projects/B/mind.yaml"), "schema_version: '1'\n").unwrap();
    let manifest = std::fs::read_to_string(dir.path().join("minds.yaml")).unwrap();
    std::fs::write(dir.path().join("minds.yaml"), format!("{manifest}- projects/B\n")).unwrap();
    let (created, create_err, create_code) =
        mf_at(&["article", "new", "alpha", "--project", "B"], dir.path(), dir.path(), true);
    assert_eq!(create_code, 0, "fixture article creation failed: {create_err} {created}");

    let (stdout, stderr, code) = mf_at(&["article", "list", "--project", "B"], dir.path(), &project, true);
    assert_ne!(code, 0, "ambiguous bare name must be refused, not silently resolved: stdout={stdout} stderr={stderr}");
    let error_output = if stderr.trim().is_empty() { stdout } else { stderr };
    let v: serde_json::Value = serde_json::from_str(&error_output).unwrap();
    assert_eq!(v["status"], "error");
    let msg = v["error"]["message"].as_str().unwrap_or_default();
    assert!(msg.contains("ambiguous") || msg.contains("both"), "message should name the ambiguity: {msg}");

    // The `./`-prefixed form is unambiguous and must still resolve cwd-relative.
    let (stdout2, stderr2, code2) = mf_at(&["article", "list", "--project", "./B"], dir.path(), &project, false);
    assert_eq!(code2, 0, "./B should resolve cwd-relative without ambiguity: stderr={stderr2}");
    assert!(!stdout2.contains("alpha"), "./B must resolve to the empty cwd child, not projects/B: {stdout2}");
}

#[test]
fn project_selector_outside_repo_boundary_is_refused() {
    let (dir, project) = setup_nonflat_repo_with_project();
    let outside = TempDir::new().unwrap();
    let escape = outside.path().to_str().unwrap();
    let (stdout, stderr, code) = mf_at(&["article", "list", "--project", escape], dir.path(), &project, true);
    assert_ne!(code, 0, "an absolute path outside the repo must be refused: stdout={stdout} stderr={stderr}");
    let error_output = if stderr.trim().is_empty() { stdout } else { stderr };
    let v: serde_json::Value = serde_json::from_str(&error_output).unwrap();
    assert_eq!(v["status"], "error");
}

#[test]
fn project_dot_refusal_matches_text_and_json_error_shape() {
    let (dir, project) = setup_nonflat_repo_with_project();
    std::fs::create_dir_all(project.parent().unwrap().join("not-a-project")).unwrap();

    let (_, stderr_text, code_text) =
        mf_at(&["article", "index", "--project", "not-a-project"], dir.path(), project.parent().unwrap(), false);
    let (stdout_json, stderr_json, code_json) =
        mf_at(&["article", "index", "--project", "not-a-project"], dir.path(), project.parent().unwrap(), true);

    assert_eq!(code_text, code_json, "text and JSON exit codes must match");
    assert_ne!(code_text, 0);
    let error_output = if stderr_json.trim().is_empty() { stdout_json } else { stderr_json };
    let v: serde_json::Value = serde_json::from_str(&error_output).unwrap();
    assert_eq!(v["status"], "error");
    let message = v["error"]["message"].as_str().unwrap_or_default();
    let hint = v["error"]["hint"].as_str().unwrap_or_default();
    assert_eq!(v["error"]["kind"], "usage");
    assert!(!hint.is_empty(), "JSON error must include a hint: {v}");
    assert!(!stderr_text.trim().is_empty(), "text mode must emit a diagnostic on stderr");
    assert!(stderr_text.contains(message), "text and JSON error messages diverged: {stderr_text} vs {message}");
    assert!(stderr_text.contains(hint), "text mode must include the JSON hint: {stderr_text} vs {hint}");
}
