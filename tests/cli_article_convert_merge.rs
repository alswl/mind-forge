//! `mf article convert --to-single-file --merge` — spec 064 Bug #20
//! (contracts/article-convert-merge.md).

use assert_cmd::Command;
use std::fs;

mod common;

fn mf() -> Command {
    Command::cargo_bin("mf").expect("binary exists")
}

fn write_index_entry(repo: &common::TempDir, project: &str, article: &str, article_path: &str) {
    let yaml = format!(
        "schema: '1'\narticles:\n  - title: {article}\n    project: {project}\n    type: blog\n    article_path: {article_path}\n    status: draft\n    created_at: '2026-07-01T00:00:00Z'\n    updated_at: '2026-07-01T00:00:00Z'\n"
    );
    fs::write(repo.path().join(project).join("mind-index.yaml"), yaml).unwrap();
}

#[test]
fn convert_without_merge_still_skips_multi_block_directory() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    let article_dir = repo.path().join("my-project/docs/my-article");
    fs::create_dir_all(&article_dir).unwrap();
    fs::write(article_dir.join("01-opening.md"), "# Opening\n").unwrap();
    fs::write(article_dir.join("02-body.md"), "## Body\n").unwrap();
    write_index_entry(&repo, "my-project", "my-article", "docs/my-article");

    let output = mf()
        .current_dir(repo.path().join("my-project"))
        .args(["article", "convert", "--to-single-file"])
        .output()
        .expect("command runs");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("multiple_section_files"), "stdout: {stdout}");
    assert!(article_dir.exists(), "source directory must be untouched");
}

#[test]
fn convert_merge_flag_with_to_directory_is_usage_error() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let output = mf()
        .current_dir(repo.path().join("my-project"))
        .args(["article", "convert", "--to-directory", "--merge"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(2), "--merge requires --to-single-file");
}

#[test]
fn convert_merge_flag_without_any_direction_is_usage_error() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    let output = mf()
        .current_dir(repo.path().join("my-project"))
        .args(["article", "convert", "--merge"])
        .output()
        .expect("command runs");
    assert_eq!(output.status.code(), Some(2), "--merge alone (no direction flag) is a usage error");
}

#[test]
fn convert_merge_concatenates_blocks_strips_frontmatter_and_rewrites_assets() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    let project_path = repo.path().join("my-project");
    let article_dir = project_path.join("docs/my-article");
    fs::create_dir_all(&article_dir).unwrap();
    fs::create_dir_all(project_path.join("assets")).unwrap();
    fs::write(
        article_dir.join("01-opening.md"),
        "---\ntypora-copy-images-to: ../assets\n---\n# Opening\n\n![hero](../../assets/pic.png)\n",
    )
    .unwrap();
    fs::write(
        article_dir.join("02-body.md"),
        "---\ntypora-copy-images-to: ../assets\n---\n## Body\n\n<img src=\"../../assets/pic.png\">\n",
    )
    .unwrap();
    write_index_entry(&repo, "my-project", "my-article", "docs/my-article");

    let output = mf()
        .current_dir(&project_path)
        .args(["article", "convert", "--to-single-file", "--merge"])
        .output()
        .expect("command runs");
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    // Source directory removed, single file created.
    assert!(!article_dir.exists(), "source directory should be removed");
    let target = project_path.join("docs/my-article.md");
    assert!(target.exists(), "merged single file should exist");
    let content = fs::read_to_string(&target).unwrap();

    // Order preserved: opening before body.
    let opening_pos = content.find("# Opening").unwrap();
    let body_pos = content.find("## Body").unwrap();
    assert!(opening_pos < body_pos, "blocks must be merged in filename order: {content}");

    // Typora frontmatter key stripped from both blocks.
    assert!(!content.contains("typora-copy-images-to"), "typora frontmatter must be stripped: {content}");

    // Asset refs re-depthed from block depth (../../assets) to file depth (../assets).
    assert!(content.contains("![hero](../assets/pic.png)"), "markdown ref must be re-depthed: {content}");
    assert!(content.contains(r#"<img src="../assets/pic.png">"#), "html img ref must be re-depthed: {content}");
}

#[test]
fn convert_merge_updates_index_and_rebinds_prompt() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    let project_path = repo.path().join("my-project");
    let article_dir = project_path.join("docs/my-article");
    fs::create_dir_all(&article_dir).unwrap();
    fs::write(article_dir.join("01-opening.md"), "# Opening\n").unwrap();
    fs::write(article_dir.join("02-body.md"), "## Body\n").unwrap();
    write_index_entry(&repo, "my-project", "my-article", "docs/my-article");

    fs::create_dir_all(project_path.join("prompts")).unwrap();
    fs::write(
        project_path.join("prompts/my-article.md"),
        "---\narticle: docs/my-article\n---\n\nWrite about my-article.\n",
    )
    .unwrap();

    let output = mf()
        .current_dir(&project_path)
        .args(["article", "convert", "--to-single-file", "--merge"])
        .output()
        .expect("command runs");
    assert!(output.status.success());

    // mind-index.yaml article_path updated.
    let index_content = fs::read_to_string(project_path.join("mind-index.yaml")).unwrap();
    assert!(index_content.contains("docs/my-article.md"), "index should point at the new path: {index_content}");
    assert!(!index_content.contains("article_path: docs/my-article\n"), "index should not keep the old path");

    // Prompt frontmatter rebound to the new path, prompt file itself unchanged in location.
    let prompt_content = fs::read_to_string(project_path.join("prompts/my-article.md")).unwrap();
    assert!(prompt_content.contains("article: docs/my-article.md"), "prompt: {prompt_content}");
}

#[test]
fn convert_merge_json_result_carries_merged_section_files() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    let project_path = repo.path().join("my-project");
    let article_dir = project_path.join("docs/my-article");
    fs::create_dir_all(&article_dir).unwrap();
    fs::write(article_dir.join("01-opening.md"), "# Opening\n").unwrap();
    fs::write(article_dir.join("02-body.md"), "## Body\n").unwrap();
    write_index_entry(&repo, "my-project", "my-article", "docs/my-article");

    let output = mf()
        .current_dir(&project_path)
        .args(["article", "convert", "--to-single-file", "--merge", "--json"])
        .output()
        .expect("command runs");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");

    let converted = json["data"]["converted"].as_array().expect("converted array");
    assert_eq!(converted.len(), 1);
    let merged = converted[0]["merged_section_files"].as_array().expect("merged_section_files array");
    assert_eq!(merged.len(), 2);
    assert_eq!(merged[0], "docs/my-article/01-opening.md");
    assert_eq!(merged[1], "docs/my-article/02-body.md");
}

#[test]
fn convert_merge_dry_run_writes_nothing() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    let project_path = repo.path().join("my-project");
    let article_dir = project_path.join("docs/my-article");
    fs::create_dir_all(&article_dir).unwrap();
    fs::write(article_dir.join("01-opening.md"), "# Opening\n").unwrap();
    fs::write(article_dir.join("02-body.md"), "## Body\n").unwrap();
    write_index_entry(&repo, "my-project", "my-article", "docs/my-article");

    let output = mf()
        .current_dir(&project_path)
        .args(["article", "convert", "--to-single-file", "--merge", "--dry-run"])
        .output()
        .expect("command runs");
    assert!(output.status.success());

    assert!(article_dir.exists(), "dry-run must not remove the source directory");
    assert!(article_dir.join("01-opening.md").exists());
    assert!(!project_path.join("docs/my-article.md").exists(), "dry-run must not write the target file");
}

#[test]
fn convert_merge_still_blocked_by_extra_files() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    let project_path = repo.path().join("my-project");
    let article_dir = project_path.join("docs/my-article");
    fs::create_dir_all(&article_dir).unwrap();
    fs::write(article_dir.join("01-opening.md"), "# Opening\n").unwrap();
    fs::write(article_dir.join("02-body.md"), "## Body\n").unwrap();
    fs::write(article_dir.join("notes.txt"), "not markdown\n").unwrap();
    write_index_entry(&repo, "my-project", "my-article", "docs/my-article");

    let output = mf()
        .current_dir(&project_path)
        .args(["article", "convert", "--to-single-file", "--merge"])
        .output()
        .expect("command runs");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("extra_files"), "stdout: {stdout}");
    assert!(article_dir.exists(), "source directory must be untouched when blocked");
}

#[test]
fn convert_merge_still_blocked_by_target_exists() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");
    let project_path = repo.path().join("my-project");
    let article_dir = project_path.join("docs/my-article");
    fs::create_dir_all(&article_dir).unwrap();
    fs::write(article_dir.join("01-opening.md"), "# Opening\n").unwrap();
    fs::write(article_dir.join("02-body.md"), "## Body\n").unwrap();
    fs::write(project_path.join("docs/my-article.md"), "# Already exists\n").unwrap();
    write_index_entry(&repo, "my-project", "my-article", "docs/my-article");

    let output = mf()
        .current_dir(&project_path)
        .args(["article", "convert", "--to-single-file", "--merge"])
        .output()
        .expect("command runs");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("target_exists"), "stdout: {stdout}");
    assert!(article_dir.exists(), "source directory must be untouched when blocked");
}
