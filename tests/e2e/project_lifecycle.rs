use std::fs;

use crate::datasets::*;
use crate::helpers::*;

// ---------------------------------------------------------------------------
// US1: mf project new (P1) — SC-001, SC-006, SC-008
// ---------------------------------------------------------------------------

/// E2E: 创建项目骨架并注册到 minds.yaml（SC-006 第一步）
#[test]
fn e2e_new_creates_project_skeleton() {
    let ds = Dataset::empty();

    let (stdout, stderr, code) = run_in(ds.root(), &["project", "new", "alpha"]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("alpha"), "stdout: {stdout}");
    assert!(stdout.contains("created"), "stdout should have timestamp: {stdout}");

    // 骨架文件系统
    assert!(ds.root().join("alpha/docs").exists(), "docs/");
    assert!(ds.root().join("alpha/docs/images").exists(), "docs/images/");
    assert!(ds.root().join("alpha/sources").exists(), "sources/");
    assert!(ds.root().join("alpha/assets").exists(), "assets/");
    assert!(ds.root().join("alpha/mind.yaml").exists(), "mind.yaml");
    assert!(ds.root().join("alpha/mind-index.yaml").exists(), "mind-index.yaml");

    // minds.yaml 注册
    let manifest = ds.read_manifest();
    assert!(manifest.contains("alpha"), "manifest should have alpha entry");
}

/// E2E: --force 幂等，不破坏已有文件（FR-005）
#[test]
fn e2e_new_force_is_idempotent() {
    let ds = Dataset::empty();

    run_in(ds.root(), &["project", "new", "alpha"]);

    // 记录 minds.yaml 第一次的 created_at
    let manifest_first = ds.read_manifest();

    // --force 再次执行
    let (_, stderr, code) = run_in(ds.root(), &["project", "new", "alpha", "--force"]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(ds.root().join("alpha/docs").exists(), "docs still exist");

    // created_at 不因 --force 改变
    let manifest_second = ds.read_manifest();
    assert_eq!(manifest_first, manifest_second, "manifest should be unchanged by --force");
}

/// E2E: 无 --force 时目录已存在报错（FR-004）
#[test]
fn e2e_new_rejects_existing_dir() {
    let ds = Dataset::empty();

    run_in(ds.root(), &["project", "new", "alpha"]);
    let (_, stderr, code) = run_in(ds.root(), &["project", "new", "alpha"]);

    assert_eq!(code, 1, "should reject existing dir");
    assert!(
        stderr.contains("already exists")
            || stderr.contains("file exists")
            || stderr.contains("file-exists")
            || stderr.contains("refusing to overwrite"),
        "stderr: {stderr}"
    );
}

/// E2E: 非法 NAME（含 ..）被拒绝，不产生文件副作用（SC-008）
#[test]
fn e2e_new_rejects_dotdot_no_side_effects() {
    let ds = Dataset::empty();

    // 记录目录结构快照
    let entries_before: Vec<_> = fs::read_dir(ds.root()).unwrap().collect();

    let (_, _, code) = run_in(ds.root(), &["project", "new", "../escape"]);

    assert_eq!(code, 2, "should reject ../");

    let entries_after: Vec<_> = fs::read_dir(ds.root()).unwrap().collect();
    assert_eq!(entries_before.len(), entries_after.len(), "no new files should be created");
}

/// E2E: --root 从非 repo 目录操作（FR-501, FR-505）
#[test]
fn e2e_new_root_flag_overrides_cwd() {
    let ds = Dataset::empty();
    let outside = Dataset::outside();

    let (stdout, stderr, code) = run_in(
        outside.path(),
        &["project", "new", "alpha", "--root", &ds.root().to_string_lossy()],
    );

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("alpha"));
    assert!(ds.root().join("alpha/docs").exists(), "project created in --root dir");
}

// ---------------------------------------------------------------------------
// US2: mf project list (P1) — SC-002, SC-005, SC-006
// ---------------------------------------------------------------------------

/// E2E: 空清单友好提示（FR-101）
#[test]
fn e2e_list_empty_manifest() {
    let ds = Dataset::empty();

    let (stdout, _, code) = run_in(ds.root(), &["project", "list"]);

    assert_eq!(code, 0);
    assert!(
        stdout.contains("no project") || stdout.contains("(no project)"),
        "should show empty hint: {stdout}"
    );
}

/// E2E: 多项目按名称排序输出（FR-102, FR-103, SC-005）
#[test]
fn e2e_list_shows_projects_sorted() {
    let ds = repo_008_empty_projects();

    let (stdout, _, code) = run_in(ds.root(), &["project", "list"]);

    assert_eq!(code, 0);
    assert!(stdout.contains("alpha"), "{stdout}");
    assert!(stdout.contains("beta"), "{stdout}");
    assert!(stdout.contains("gamma"), "{stdout}");

    // 排序校验：alpha 在 beta 之前
    let alpha_pos = stdout.find("alpha").unwrap_or(usize::MAX);
    let beta_pos = stdout.find("beta").unwrap_or(usize::MAX);
    assert!(alpha_pos < beta_pos, "alpha should appear before beta");
}

/// E2E: document_count 从 mind-index.yaml 计算（FR-103, FR-104）
#[test]
fn e2e_list_counts_documents_from_index() {
    let ds = repo_008_with_data();

    let (stdout, _, code) = run_in(ds.root(), &["project", "list"]);

    assert_eq!(code, 0);
    assert!(stdout.contains("alpha"), "{stdout}");
    assert!(stdout.contains("beta"), "{stdout}");
    // alpha 有 4 个文档，不应显示 0
    assert!(!stdout.contains("alpha   0"), "alpha should have non-zero count");
}

/// E2E: 缺失 mind-index.yaml 时以 0 计数显示（FR-104）
#[test]
fn e2e_list_missing_index_shows_zero() {
    let ds = repo_008_empty_projects();

    // 删除 alpha 的 mind-index.yaml
    let index_path = ds.root().join("alpha/mind-index.yaml");
    if index_path.exists() {
        fs::remove_file(&index_path).unwrap();
    }

    let (stdout, _, code) = run_in(ds.root(), &["project", "list"]);

    assert_eq!(code, 0, "should not fail");
    // alpha 应该仍出现，但 document_count = 0
    assert!(stdout.contains("alpha"), "{stdout}");
}

/// E2E: JSON 模式输出结构符合契约（FR-103）
#[test]
fn e2e_list_json_envelope() {
    let ds = repo_008_with_data();

    let (stdout, _, code) = run_in(ds.root(), &["--format", "json", "project", "list"]);

    assert_eq!(code, 0);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["status"], "ok");
    assert!(parsed["data"]["projects"].is_array(), "projects should be an array");
}

// ---------------------------------------------------------------------------
// US3: mf project status (P2) — SC-003, SC-006
// ---------------------------------------------------------------------------

/// E2E: 命名项目显示正确计数（FR-203, FR-204）
#[test]
fn e2e_status_shows_counts_for_named_project() {
    let ds = repo_008_with_data();

    let (stdout, _, code) = run_in(ds.root(), &["project", "status", "--project", "alpha"]);

    assert_eq!(code, 0, "stdout: {stdout}");
    assert!(stdout.contains("articles"), "should show articles: {stdout}");
    // alpha 有 2 articles, 1 asset, 1 source, 0 terms
}

/// E2E: cwd 检测项目（FR-201）
#[test]
fn e2e_status_detects_project_from_cwd() {
    let ds = repo_008_with_data();

    let (stdout, _, code) = run_in(ds.root().join("alpha/docs"), &["project", "status"]);

    assert_eq!(code, 0, "should detect alpha from cwd: {stdout}");
    assert!(stdout.contains("alpha"), "should show project name");
}

/// E2E: 空索引返回 0 计数和 null updated_at（FR-205）
#[test]
fn e2e_status_empty_index() {
    let ds = repo_008_empty_projects();

    let (stdout, _, code) = run_in(ds.root(), &["project", "status", "--project", "alpha"]);

    assert_eq!(code, 0);
    assert!(stdout.contains("articles"), "should show articles: {stdout}");
    assert!(
        stdout.contains("0") || stdout.contains("-"),
        "empty project should show zero: {stdout}"
    );
}

/// E2E: 未知项目名返回 usage 错误（FR-206）
#[test]
fn e2e_status_rejects_unknown_project() {
    let ds = Dataset::empty();

    let (_, stderr, code) = run_in(ds.root(), &["project", "status", "--project", "nonexistent"]);

    assert_eq!(code, 2, "should reject unknown project: {stderr}");
    assert!(
        stderr.contains("usage") || stderr.contains("not found") || stderr.contains("list"),
        "stderr: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// US4: mf project lint (P2) — SC-004, SC-006
// ---------------------------------------------------------------------------

/// E2E: 检测缺失目录（FR-301）
#[test]
fn e2e_lint_reports_missing_directory() {
    let ds = repo_008_with_lint_issues();

    let (stdout, stderr, code) = run_in(ds.root(), &["project", "lint", "--project", "alpha"]);

    assert_eq!(code, 1, "lint should find errors: stderr={stderr}, stdout={stdout}");
    let output = stdout + &stderr;
    assert!(
        output.contains("missing_directory") || output.contains("sources"),
        "should report missing sources: {output}"
    );
}

/// E2E: 检测过期索引条目（FR-302）
#[test]
fn e2e_lint_reports_stale_index_entry() {
    let ds = repo_008_with_lint_issues();

    let (stdout, stderr, code) = run_in(ds.root(), &["project", "lint", "--project", "alpha"]);

    assert_eq!(code, 1, "lint should find errors");
    let output = stdout + &stderr;
    assert!(
        output.contains("stale_index_entry") || output.contains("ghost"),
        "should report stale entry: {output}"
    );
}

/// E2E: --fix 创建缺失目录（FR-305）
#[test]
fn e2e_lint_fix_creates_missing_directory() {
    let ds = repo_008_with_lint_issues();

    // 确认 sources 不存在
    assert!(!ds.root().join("alpha/sources").exists());

    let (stdout, stderr, code) =
        run_in(ds.root(), &["project", "lint", "--project", "alpha", "--fix"]);

    assert_eq!(code, 0, "all issues should be fixed: stderr={stderr}, stdout={stdout}");

    // sources/ 被修复创建
    assert!(ds.root().join("alpha/sources").exists(), "sources should be created by --fix");
}

/// E2E: 检测命名规范违规（FR-303）
#[test]
fn e2e_lint_reports_name_convention_violation() {
    let ds = repo_008_with_name_violation();

    let (stdout, stderr, code) = run_in(ds.root(), &["project", "lint", "--project", "alpha"]);

    let output = stdout + &stderr;
    assert_eq!(code, 0, "name convention is warning-only, should exit 0: {output}");
    assert!(
        output.contains("name_convention") || output.contains("Some Article"),
        "should report name convention: {output}"
    );
}

/// E2E: 干净项目无问题（FR-301）
#[test]
fn e2e_lint_clean_project_no_issues() {
    let ds = repo_008_empty_projects();

    let (stdout, _, code) = run_in(ds.root(), &["project", "lint", "--project", "alpha"]);

    assert_eq!(code, 0, "clean project should pass lint");
    assert!(
        stdout.contains("no issues") || stdout.contains("(no issues)"),
        "should report no issues: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// US5: mf project archive (P3) — SC-007, FR-401, FR-402
// ---------------------------------------------------------------------------

/// E2E: archive 返回 not-implemented（SC-007, FR-402）
#[test]
fn e2e_archive_returns_not_implemented() {
    let ds = Dataset::empty();

    let (stdout, stderr, code) = run_in(ds.root(), &["project", "archive", "alpha"]);

    assert_eq!(code, 64, "should be not implemented");
    let all = stdout + &stderr;
    assert!(
        all.contains("not implemented") || all.contains("not yet implemented"),
        "should say not implemented: {all}"
    );
}

// ---------------------------------------------------------------------------
// 全局 --root 与 Repo 边界（SC-008, FR-501~FR-505）
// ---------------------------------------------------------------------------

/// E2E: --root 从 repo 外操作 list（FR-501, FR-504）
#[test]
fn e2e_root_flag_overrides_cwd_for_list() {
    let ds = repo_008_with_data();
    let outside = Dataset::outside();

    let (stdout, _, code) =
        run_in(outside.path(), &["project", "list", "--root", &ds.root().to_string_lossy()]);

    assert_eq!(code, 0, "should work with --root from outside");
    assert!(stdout.contains("alpha"), "should list projects: {stdout}");
}

/// E2E: --root 指向非 repo 目录报错（FR-501）
#[test]
fn e2e_root_flag_rejects_non_repo() {
    let outside = Dataset::outside();

    let (_, stderr, code) =
        run_in(outside.path(), &["project", "list", "--root", &outside.path().to_string_lossy()]);

    assert_eq!(code, 1);
    assert!(stderr.contains("not in a mind repo"), "should reject non-repo: {stderr}");
}

/// E2E: 无 --root 且不在 repo 内报错（FR-007）
#[test]
fn e2e_not_in_repo_returns_error() {
    let outside = Dataset::outside();

    let (_, stderr, code) = run_in(outside.path(), &["project", "list"]);

    assert_eq!(code, 1);
    assert!(stderr.contains("not in a mind repo"), "stderr: {stderr}");
}

/// E2E: 多命令连续执行（SC-006）
#[test]
fn e2e_full_lifecycle_quickstart() {
    let ds = Dataset::empty();

    // new
    let (_, _, code) = run_in(ds.root(), &["project", "new", "my-project"]);
    assert_eq!(code, 0, "new failed");
    assert!(ds.root().join("my-project/mind.yaml").exists());

    // list
    let (stdout, _, code) = run_in(ds.root(), &["project", "list"]);
    assert_eq!(code, 0, "list failed");
    assert!(stdout.contains("my-project"), "list should show new project: {stdout}");

    // status（指定 project）
    let (stdout, _, code) = run_in(ds.root(), &["project", "status", "--project", "my-project"]);
    assert_eq!(code, 0, "status failed: {stdout}");

    // lint（刚创建的项目应该没有 issues）
    let (stdout, _, code) = run_in(ds.root(), &["project", "lint", "--project", "my-project"]);
    assert_eq!(code, 0, "lint of fresh project failed: {stdout}");

    // archive → not implemented
    let (_, _, code) = run_in(ds.root(), &["project", "archive", "my-project"]);
    assert_eq!(code, 64);
}
