use std::fs;

use crate::datasets::{self, Dataset};
use crate::helpers::*;

// ---------------------------------------------------------------------------
// mf project index — E2E 场景
// ---------------------------------------------------------------------------

/// E2E: 创建一个项目后 index，minds.yaml 应包含该项目的 projects 条目
#[test]
fn index_discovers_new_project() {
    let ds = Dataset::empty().with_project("my-doc");

    let (_, _, code) = run_in(ds.root(), &["project", "index"]);
    assert_eq!(code, 0, "exit 0 for implemented command");

    let content = ds.read_manifest();
    assert!(content.contains("my-doc"), "manifest should contain new project: {content}");
    assert!(content.contains("projects"), "manifest should have projects key");
}

/// E2E: 删除项目目录后 index，minds.yaml 应移除该条目
#[test]
fn index_removes_deleted_project() {
    let ds = Dataset::empty().with_project("to-go");

    // 先注册项目
    run_in(ds.root(), &["project", "index"]);
    assert!(ds.read_manifest().contains("to-go"));

    // 删除目录并重新 index
    fs::remove_dir_all(ds.root().join("projects").join("to-go")).unwrap();
    run_in(ds.root(), &["project", "index"]);

    let content = ds.read_manifest();
    assert!(!content.contains("to-go"), "removed project should be gone: {content}");
}

/// E2E: 多个项目应全部注册
#[test]
fn index_discovers_multiple_projects() {
    let ds = datasets::repo_with_three_projects();

    run_in(ds.root(), &["project", "index"]);
    let content = ds.read_manifest();

    assert!(content.contains("alpha"), "{content}");
    assert!(content.contains("beta"), "{content}");
    assert!(content.contains("gamma"), "{content}");
}

/// E2E: 不含 mind.yaml 的目录应被忽略
#[test]
fn index_ignores_non_project_dirs() {
    let ds = datasets::repo_with_mixed_content();

    run_in(ds.root(), &["project", "index"]);
    let content = ds.read_manifest();

    assert!(content.contains("real-project"));
    assert!(!content.contains("just-a-folder"), "non-project should be ignored: {content}");
    assert!(!content.contains("another-folder"), "non-project should be ignored: {content}");
}

/// E2E: dry-run 模式不修改文件
#[test]
fn index_dry_run_does_not_modify() {
    let ds = Dataset::empty();
    let before = ds.read_manifest();

    fs::create_dir_all(ds.root().join("projects/new-project")).unwrap();
    fs::write(ds.root().join("projects/new-project/mind.yaml"), "schema_version: '1'\n").unwrap();
    run_in(ds.root(), &["project", "index", "--dry-run"]);

    let after = ds.read_manifest();
    assert_eq!(before, after, "dry-run should not modify minds.yaml");
}

/// E2E: minds.yaml 不存在时自动创建
#[test]
fn index_creates_minds_yaml_when_absent() {
    let dir = Dataset::outside();
    assert!(!dir.path().join("minds.yaml").exists());

    fs::create_dir_all(dir.path().join("projects/new-project")).unwrap();
    fs::write(dir.path().join("projects/new-project/mind.yaml"), "schema_version: '1'\n").unwrap();

    run_in(dir.path(), &["project", "index"]);

    assert!(dir.path().join("minds.yaml").exists(), "minds.yaml should be created");
    let content = std::fs::read_to_string(dir.path().join("minds.yaml")).unwrap();
    assert!(content.contains("new-project"));
}

/// E2E: 文件为普通文件而非目录时的行为
#[test]
fn index_skips_files_not_dirs() {
    let ds = Dataset::empty();
    fs::write(ds.root().join("readme.md"), "hello").unwrap();

    run_in(ds.root(), &["project", "index"]);
    let content = ds.read_manifest();
    assert!(!content.contains("readme"));
}

/// E2E: schema_version 不兼容时报错
#[test]
fn index_rejects_incompatible_schema() {
    let ds = Dataset::incompatible_schema().with_project("p1");

    let (_, stderr, code) = run_in(ds.root(), &["project", "index"]);
    assert_eq!(code, 2);
    assert!(stderr.contains("incompatible schema"));
}

/// E2E: --output json 时 project index 输出为 JSON
#[test]
fn index_json_output_format() {
    let ds = Dataset::empty().with_project("json-project");

    let (stdout, _, code) = run_in(ds.root(), &["--output", "json", "project", "index"]);
    assert_eq!(code, 0);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["status"], "ok");
    assert!(parsed["data"]["kept_count"].is_number());
}

/// E2E: 连续两次 index 是幂等的
#[test]
fn index_is_idempotent() {
    let ds = Dataset::empty().with_project("stable");

    run_in(ds.root(), &["project", "index"]);
    let after_first = ds.read_manifest();

    run_in(ds.root(), &["project", "index"]);
    let after_second = ds.read_manifest();

    assert_eq!(after_first, after_second, "index should be idempotent");
}

// ---------------------------------------------------------------------------
// spec 082 US1 (#55): `--project .` must agree across command families
// ---------------------------------------------------------------------------

/// E2E: `--project .` resolves identically for `article index`, `project
/// lint`, and `build`, run from inside the project directory. Before the
/// fix, `article index --project .` returned `status: ok` with a zero scan
/// (it resolved to the projects directory, not the project), while `project
/// lint --project .` failed outright with "project '.' not found" — the
/// same selector, three different outcomes across three commands.
#[test]
fn project_dot_agrees_across_article_index_project_lint_and_build() {
    let ds = Dataset::empty().with_project("crossfam");
    let project_dir = ds.root().join("projects/crossfam");
    std::fs::create_dir_all(project_dir.join("docs/x")).unwrap();
    std::fs::create_dir_all(project_dir.join("sources")).unwrap();
    std::fs::create_dir_all(project_dir.join("assets")).unwrap();
    std::fs::write(project_dir.join("docs/x/01-opening.md"), "# Opening\n").unwrap();

    // article index: --project . must scan the same as the bare form.
    let (stdout_dot, stderr_dot, code_dot) = run_in(&project_dir, &["--json", "article", "index", "--project", "."]);
    assert_eq!(code_dot, 0, "stderr: {stderr_dot}");
    let dot: serde_json::Value = serde_json::from_str(&stdout_dot).unwrap();

    let (stdout_bare, stderr_bare, code_bare) = run_in(&project_dir, &["--json", "article", "index"]);
    assert_eq!(code_bare, 0, "stderr: {stderr_bare}");
    let bare: serde_json::Value = serde_json::from_str(&stdout_bare).unwrap();

    assert_eq!(
        dot["data"]["scanned_count"], bare["data"]["scanned_count"],
        "article index --project . diverged from the bare form: dot={dot} bare={bare}"
    );
    assert_eq!(dot["data"]["scanned_count"], 1, "expected the one fixture article to be scanned: {dot}");

    // project lint: --project . must reach the same project as the bare
    // form (both may still report lint findings — that's not what's under
    // test here, only that the *resolution* agrees).
    let (_, stderr_lint_dot, code_lint_dot) = run_in(&project_dir, &["project", "lint", "--project", "."]);
    let (_, stderr_lint_bare, code_lint_bare) = run_in(&project_dir, &["project", "lint"]);
    assert_eq!(
        code_lint_dot, code_lint_bare,
        "project lint --project . vs bare form diverged: dot(stderr={stderr_lint_dot}) bare(stderr={stderr_lint_bare})"
    );

    // build: --project . must resolve the same article set as the bare form.
    let (_, stderr_build_dot, code_build_dot) = run_in(&project_dir, &["build", "docs/x", "--project", "."]);
    assert_eq!(code_build_dot, 0, "build --project . should succeed: stderr={stderr_build_dot}");
    assert!(
        project_dir.join("outputs/x.md").exists() || project_dir.join("build/x.md").exists(),
        "build --project . should have produced an artifact"
    );
}
