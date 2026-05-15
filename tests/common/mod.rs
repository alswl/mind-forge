use std::fs;
use std::path::{Path, PathBuf};

pub use tempfile::TempDir;

/// 创建一个临时 Mind Repo（扁平 layout：projects_dir: "."）
#[allow(dead_code)]
pub fn setup_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("minds.yaml"), "schema_version: '1'\nprojects_dir: '.'\nprojects: []\n").unwrap();
    dir
}

/// 在 repo 中创建一个项目目录（含 mind.yaml）
#[allow(dead_code)]
pub fn create_project(repo: &TempDir, name: &str) {
    let project_dir = repo.path().join(name);
    fs::create_dir_all(&project_dir).unwrap();
    fs::write(project_dir.join("mind.yaml"), "schema_version: '1'\n").unwrap();
}

/// 在项目目录中写入 mind-index.yaml
#[allow(dead_code)]
pub fn write_index(repo: &TempDir, name: &str, yaml: &str) {
    let index_path = repo.path().join(name).join("mind-index.yaml");
    if let Some(parent) = index_path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&index_path, yaml).unwrap();
}

/// 写入 `<project>/_build/<article>.<format>`，返回完整路径。
#[allow(dead_code)]
pub fn write_build_artifact(project: &Path, article: &str, format: &str, content: &[u8]) -> PathBuf {
    let build_dir = project.join("_build");
    fs::create_dir_all(&build_dir).unwrap();
    let path = build_dir.join(format!("{article}.{format}"));
    fs::write(&path, content).unwrap();
    path
}

/// 完全覆盖项目的 `mind.yaml` 为给定内容。
#[allow(dead_code)]
pub fn write_mind_yaml(repo: &TempDir, project_name: &str, yaml: &str) {
    let project_dir = repo.path().join(project_name);
    fs::create_dir_all(&project_dir).unwrap();
    fs::write(project_dir.join("mind.yaml"), yaml).unwrap();
}

/// 在 mind-index.yaml 中追加一条文章索引（最简形式，复用既有 schema）。
#[allow(dead_code)]
pub fn write_article_index(repo: &TempDir, project_name: &str, article: &str) {
    let yaml = format!(
        "schema_version: '1'\narticles:\n  - title: '{article}'\n    project: '{project_name}'\n    article_type: blog\n    source_path: 'docs/{article}.md'\n    status: draft\n    created_at: '2026-05-07T00:00:00Z'\n    updated_at: '2026-05-07T00:00:00Z'\n"
    );
    write_index(repo, project_name, &yaml);
}

/// 在项目 docs/ 目录中写入 Markdown 文件。
#[allow(dead_code)]
pub fn write_doc(repo: &TempDir, project_name: &str, name: &str, content: &str) {
    let doc_dir = repo.path().join(project_name).join("docs");
    fs::create_dir_all(&doc_dir).unwrap();
    fs::write(doc_dir.join(format!("{name}.md")), content).unwrap();
}

/// 在项目的指定相对路径（如自定义 source_dir）中写入 Markdown 文件。
#[allow(dead_code)]
pub fn write_source_file(repo: &TempDir, project_name: &str, rel_dir: &str, name: &str, content: &str) {
    let dst_dir = repo.path().join(project_name).join(rel_dir);
    fs::create_dir_all(&dst_dir).unwrap();
    fs::write(dst_dir.join(format!("{name}.md")), content).unwrap();
}

/// 在 Mind Repo 根目录（`minds.yaml` 同级）写入 `.mind-forge/publisher/<name>.yaml`。
#[allow(dead_code)]
pub fn write_publisher_yaml(repo: &TempDir, name: &str, yaml_content: &str) {
    let publisher_dir = repo.path().join(".mind-forge").join("publisher");
    fs::create_dir_all(&publisher_dir).unwrap();
    let path = publisher_dir.join(format!("{name}.yaml"));
    fs::write(&path, yaml_content).unwrap();
}

/// 写入多个 publisher 定义，`publishers` 是 `(name, content)` 的 slice。
#[allow(dead_code)]
pub fn write_publishers(repo: &TempDir, publishers: &[(&str, &str)]) {
    for (name, content) in publishers {
        write_publisher_yaml(repo, name, content);
    }
}
/// 在 Mind Repo 根目录写入 `.mind-forge/renders/<name>.md`。
#[allow(dead_code)]
pub fn write_render_template(repo: &TempDir, name: &str, content: &str) -> PathBuf {
    let templates_dir = repo.path().join(".mind-forge").join("renders");
    fs::create_dir_all(&templates_dir).unwrap();
    let path = templates_dir.join(format!("{name}.md"));
    fs::write(&path, content).unwrap();
    path
}

/// `terms_yaml` 应为 `terms:` 段内容（不含前导空格），例如：
/// `"term: Mind Repo\n  definition: desc\n  corrections:\n    - original: mindrepo\n      correct: Mind Repo"`
#[allow(dead_code)]
pub fn write_term_index(repo: &TempDir, project_name: &str, terms_yaml: &str) {
    let yaml = format!("schema_version: '1'\nterms:\n  - {terms_yaml}\n");
    write_index(repo, project_name, &yaml);
}
