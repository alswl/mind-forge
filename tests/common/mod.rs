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

/// Create a project with all three discovery sources: generated (template),
/// declared (typed build.articles + compat articles), and docs-walked.
///
/// Mirrors the quickstart q24 fixture shape:
/// - `templates.daily_report` with `mode: generated`,
///   `pattern: "outputs/{date:YYYY-MM}/{date:YYYY-MM-DD}.md"`
///   and matching file at `outputs/2026-05/2026-05-15.md`
/// - `build.articles.reports` with `source_dir: reports` + present file
/// - compat `articles.legacy-blog` (no on-disk source → declared+missing)
/// - `docs/2026-05-10-hello.md`
/// - Two local targets: `paas-git` (with `{date:YYYY-MM}` + `prefix: "cie-"`)
///   and `simple-out` (plain path)
#[allow(dead_code)]
pub fn scaffold_three_source_project(repo: &TempDir, project_name: &str) {
    let project_path = repo.path().join(project_name);
    fs::create_dir_all(&project_path).unwrap();
    fs::create_dir_all(project_path.join("docs")).unwrap();
    fs::create_dir_all(project_path.join("reports")).unwrap();
    fs::create_dir_all(project_path.join("outputs/2026-05")).unwrap();

    let mind_yaml = format!(
        r#"schema_version: '1'
project:
  name: {name}
build:
  output_dir: _build
  format: md
  articles:
    reports:
      source_dir: reports
articles:
  legacy-blog:
    type: blog
publish:
  targets:
    - name: paas-git
      type: local
      enabled: true
      path: "/tmp/mf-024-out/{{date:YYYY-MM}}/daily/"
      prefix: "cie-"
    - name: simple-out
      type: local
      enabled: true
      path: "/tmp/mf-024-out/simple/"
templates:
  daily_report:
    pattern: "outputs/{{date:YYYY-MM}}/{{date:YYYY-MM-DD}}.md"
    mode: generated
"#,
        name = project_name,
    );
    fs::write(project_path.join("mind.yaml"), mind_yaml).unwrap();

    // Generated source
    fs::write(project_path.join("outputs/2026-05/2026-05-15.md"), b"# Daily 5-15\n").unwrap();
    // Declared+present source
    fs::write(project_path.join("reports/2026-05.md"), b"# Reports body\n").unwrap();
    // Docs-walked source
    fs::write(project_path.join("docs/2026-05-10-hello.md"), b"# Hello\n").unwrap();
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

/// Scaffold a project with a single `templates.<template_name>` entry and create the listed files.
#[allow(dead_code)]
pub fn scaffold_project_with_template(
    name: &str,
    template_name: &str,
    pattern: &str,
    mode: &str,
    files: &[&str],
) -> TempDir {
    let repo = setup_repo();
    let project_path = repo.path().join(name);
    fs::create_dir_all(&project_path).unwrap();

    let mind_yaml = format!(
        "schema_version: '1'\n\
project:\n  name: {name}\n\
build:\n  output_dir: _build\n  format: md\n\
publish:\n  targets: []\n\
templates:\n  {template_name}:\n    pattern: \"{pattern}\"\n    mode: {mode}\n",
    );
    let index_yaml = "schema_version: '1'\narticles: []\n";
    fs::write(project_path.join("mind.yaml"), mind_yaml).unwrap();
    fs::write(project_path.join("mind-index.yaml"), index_yaml).unwrap();

    // Create each file relative to the project root
    for file in files {
        let full_path = project_path.join(file);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&full_path, b"# generated\n").unwrap();
    }

    repo
}

/// `terms_yaml` 应为 `terms:` 段内容（不含前导空格），例如：
/// `"term: Mind Repo\n  definition: desc\n  corrections:\n    - original: mindrepo\n      correct: Mind Repo"`
#[allow(dead_code)]
pub fn write_term_index(repo: &TempDir, project_name: &str, terms_yaml: &str) {
    let yaml = format!("schema_version: '1'\nterms:\n  - {terms_yaml}\n");
    write_index(repo, project_name, &yaml);
}

/// Scaffold the minimal repro fixture matching quickstart.md:
/// - `projects/team-reports/docs/2026-05-monthly/01-team-okr.md`
/// - `projects/team-reports/outputs/2026-05/2026-05-15.md`
/// - build.articles `2026-05-monthly` (no source_dir)
/// - templates `daily_report` with `mode: generated`
/// - local publish target with date expansion + prefix
#[allow(dead_code)]
pub fn scaffold_team_reports_minimal_repro() -> TempDir {
    let dir = TempDir::new().unwrap();
    let repo_path = dir.path().to_path_buf();
    // Use the project as a top-level directory under the repo (matching projects_dir: '.')
    let project_name = "team-reports";
    let project_path = repo_path.join(project_name);

    // minds.yaml at repo root
    fs::write(repo_path.join("minds.yaml"), format!("schema: '1'\nprojects_dir: '.'\nprojects:\n  - {project_name}\n"))
        .unwrap();

    fs::create_dir_all(project_path.join("docs/2026-05-monthly")).unwrap();
    fs::create_dir_all(project_path.join("outputs/2026-05")).unwrap();

    // mind.yaml matching quickstart
    let mind_yaml = r#"schema: '1'
build:
  articles:
    2026-05-monthly: {}

publish:
  targets:
    - name: local-test
      type: local
      path: "/tmp/mf-repro-publish/{date:YYYY-MM}/daily/"
      prefix: "cie-"

templates:
  daily_report:
    pattern: "outputs/{date:YYYY-MM}/{date:YYYY-MM-DD}.md"
    cadence: daily
    mode: generated
"#;
    fs::write(project_path.join("mind.yaml"), mind_yaml).unwrap();

    // Create the directory article part
    fs::write(project_path.join("docs/2026-05-monthly/01-team-okr.md"), "# 2026-05 Monthly\n\nmonthly content\n")
        .unwrap();

    // Create the generated source file
    fs::write(project_path.join("outputs/2026-05/2026-05-15.md"), "# 2026-05-15\n\ndaily content\n").unwrap();

    dir
}

/// Parse `mind-index.yaml` for a project and return the articles mapping.
#[allow(dead_code)]
pub fn read_index_articles_map(repo: &TempDir, project_name: &str) -> serde_yaml::Value {
    let content = fs::read_to_string(repo.path().join(project_name).join("mind-index.yaml")).unwrap();
    let parsed: serde_yaml::Value = serde_yaml::from_str(&content).unwrap();
    parsed.get("articles").cloned().unwrap_or(serde_yaml::Value::Null)
}

/// Assert that `articles_map` contains a key with `source_path` equal to `expected`.
#[allow(dead_code)]
pub fn assert_article_source_path(articles_map: &serde_yaml::Value, key: &str, expected: &str) {
    let entry = articles_map
        .get(key)
        .unwrap_or_else(|| panic!("article key '{}' not found in articles map: {articles_map:#?}", key));
    let source_path = entry["source_path"]
        .as_str()
        .unwrap_or_else(|| panic!("article '{}' has no source_path field: {entry:#?}", key));
    assert_eq!(source_path, expected, "source_path mismatch for article '{}'", key);
}

/// Assert that `articles_map` does NOT contain a key.
#[allow(dead_code)]
pub fn assert_no_article_key(articles_map: &serde_yaml::Value, key: &str) {
    assert!(
        articles_map.get(key).is_none(),
        "article key '{}' should not exist in articles map: {articles_map:#?}",
        key
    );
}
