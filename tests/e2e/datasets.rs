use std::fs;
use std::path::Path;

use crate::helpers::TempDir;

// ---------------------------------------------------------------------------
// minds.yaml 内容常量
// ---------------------------------------------------------------------------

/// schema_version = "1" 的标准 manifest，含空 projects 列表
pub const MANIFEST_VALID: &str = "schema_version: '1'\nprojects: []\n";

/// schema_version = "999" 的不兼容 manifest
pub const MANIFEST_INCOMPATIBLE: &str = "schema_version: '999'\nprojects: []\n";

/// 非 YAML 内容，用于 parse-error 场景
pub const MANIFEST_NOT_YAML: &str = "<<<not yaml>>>";

/// 空文件
pub const MANIFEST_EMPTY: &str = "";

/// 仅含 schema_version 的有效 YAML（用于 mf.yaml 配置文件）
pub const CONFIG_VALID: &str = "schema_version: '1'\n";

/// 标准的 mind.yaml 项目标记文件
pub const MIND_YAML: &str = "schema_version: '1'\n";

// ---------------------------------------------------------------------------
// mind-index.yaml 内容常量（008 Project Lifecycle 验收用）
// ---------------------------------------------------------------------------

/// 空的 mind-index.yaml（仅 schema_version，无条目）
pub const INDEX_EMPTY: &str = "schema_version: '1'\n";

/// 含 2 篇文章 + 1 个 asset + 1 个 source 的索引，updated_at 最大值为 2026-04-30T12:15:00Z
pub const INDEX_POPULATED: &str = r#"schema_version: '1'
articles:
  - title: "First Post"
    project: "alpha"
    type: blog
    source_path: "docs/first-post.md"
    status: draft
    created_at: "2026-04-30T12:00:00Z"
    updated_at: "2026-04-30T12:00:00Z"
  - title: "Second Post"
    project: "alpha"
    type: blog
    source_path: "docs/second-post.md"
    status: published
    created_at: "2026-04-30T12:05:00Z"
    updated_at: "2026-04-30T12:15:00Z"
assets:
  - name: "logo.png"
    type: image
    path: "assets/logo.png"
    size: 102400
    hash: "d3adbeef"
    tags: []
    added_at: "2026-04-29T10:00:00Z"
sources:
  - name: "Reference"
    type: pdf
    url: ~
    path: ~
    tags: []
    added_at: "2026-04-28T10:00:00Z"
    updated_at: "2026-04-28T10:00:00Z"
"#;

/// 含一个条目指向不存在的 docs/ghost.md（用于 stale_index_entry 验收）
pub const INDEX_STALE_ENTRY: &str = r#"schema_version: '1'
articles:
  - title: "Ghost"
    project: "alpha"
    type: blog
    source_path: "docs/ghost.md"
    status: draft
    created_at: "2026-04-30T12:00:00Z"
    updated_at: "2026-04-30T12:00:00Z"
"#;

/// 含一个条目文件名违反 kebab-case（用于 name_convention 验收）
pub const INDEX_NAME_VIOLATION: &str = r#"schema_version: '1'
articles:
  - title: "Some Article"
    project: "alpha"
    type: blog
    source_path: "docs/Some Article.md"
    status: draft
    created_at: "2026-04-30T12:00:00Z"
    updated_at: "2026-04-30T12:00:00Z"
"#;

// ---------------------------------------------------------------------------
// Dataset: 目录结构构建器
// ---------------------------------------------------------------------------

/// Dataset 是一个预定义的 Mind Repo 目录结构。
/// 每个 Dataset 在 TempDir 中搭建，测试用完后自动清理。
pub struct Dataset {
    pub dir: TempDir,
}

impl Dataset {
    /// 创建一个标准空 repo（仅 minds.yaml，schema_version=1）
    pub fn empty() -> Self {
        let dir = TempDir::new().expect("temp dir");
        fs::write(dir.path().join("minds.yaml"), MANIFEST_VALID).expect("write minds.yaml");
        Self { dir }
    }

    /// 创建 minds.yaml 但不含要求的字段（空文件）
    pub fn empty_manifest() -> Self {
        let dir = TempDir::new().expect("temp dir");
        fs::write(dir.path().join("minds.yaml"), MANIFEST_EMPTY).expect("write minds.yaml");
        Self { dir }
    }

    /// 创建 schema_version 不兼容的 repo
    pub fn incompatible_schema() -> Self {
        let dir = TempDir::new().expect("temp dir");
        fs::write(dir.path().join("minds.yaml"), MANIFEST_INCOMPATIBLE).expect("write minds.yaml");
        Self { dir }
    }

    /// 创建 content 无法解析为 YAML 的 repo
    pub fn not_yaml() -> Self {
        let dir = TempDir::new().expect("temp dir");
        fs::write(dir.path().join("minds.yaml"), MANIFEST_NOT_YAML).expect("write minds.yaml");
        Self { dir }
    }

    /// 在 repo 根下创建一个含 mind.yaml 的项目目录（位于默认 `projects/<name>` 下）
    pub fn with_project(self, name: &str) -> Self {
        let dir = self.dir.path().join("projects").join(name);
        fs::create_dir_all(&dir).expect("create project dir");
        fs::write(dir.join("mind.yaml"), MIND_YAML).expect("write mind.yaml");
        self
    }

    /// 在 `projects/` 下创建不含 mind.yaml 的普通目录（用于验证 index 跳过非项目目录）
    pub fn with_non_project_dir(self, name: &str) -> Self {
        let dir = self.dir.path().join("projects").join(name);
        fs::create_dir_all(&dir).expect("create dir");
        self
    }

    /// 创建一个子目录结构
    pub fn with_subdir(self, path: &str) -> Self {
        let dir = self.dir.path().join(path);
        fs::create_dir_all(&dir).expect("create subdir");
        self
    }

    /// 在 repo 根下写一个 mf.yaml 配置文件
    pub fn with_config(self) -> Self {
        fs::write(self.dir.path().join("mf.yaml"), CONFIG_VALID).expect("write mf.yaml");
        self
    }

    /// repo 根目录
    pub fn root(&self) -> &Path {
        self.dir.path()
    }

    /// 读取 minds.yaml 内容
    pub fn read_manifest(&self) -> String {
        fs::read_to_string(self.dir.path().join("minds.yaml")).expect("read minds.yaml")
    }

    /// 在 repo 下写入 mind-index.yaml 到项目
    pub fn with_index(self, name: &str, yaml: &str) -> Self {
        let path = self.dir.path().join("projects").join(name).join("mind-index.yaml");
        fs::write(&path, yaml).expect("write mind-index.yaml");
        self
    }

    /// 创建完整骨架项目（含 docs/ sources/ assets/ mind.yaml mind-index.yaml）
    pub fn with_standard_project(self, name: &str) -> Self {
        let dir = self.dir.path().join("projects").join(name);
        fs::create_dir_all(dir.join("docs")).expect("create docs");
        fs::create_dir_all(dir.join("docs/images")).expect("create docs/images");
        fs::create_dir_all(dir.join("sources")).expect("create sources");
        fs::create_dir_all(dir.join("assets")).expect("create assets");
        fs::write(dir.join("mind.yaml"), MIND_YAML).expect("write mind.yaml");
        fs::write(dir.join("mind-index.yaml"), INDEX_EMPTY).expect("write mind-index.yaml");
        self
    }

    /// 删除项目的某个子目录（用于 lint missing_directory 验收）
    pub fn without_dir(self, name: &str, subpath: &str) -> Self {
        let path = self.dir.path().join("projects").join(name).join(subpath);
        if path.exists() {
            fs::remove_dir_all(&path).expect("remove dir");
        }
        self
    }

    /// 删除项目的 mind.yaml（用于 lint missing_manifest 验收）
    #[allow(dead_code)]
    pub fn without_manifest(self, name: &str) -> Self {
        let path = self.dir.path().join("projects").join(name).join("mind.yaml");
        if path.exists() {
            fs::remove_file(&path).expect("remove mind.yaml");
        }
        self
    }

    /// 在 repo 外创建一个临时目录（用于测试非 repo 场景）
    pub fn outside() -> TempDir {
        TempDir::new().expect("temp dir")
    }
}

// ---------------------------------------------------------------------------
// Publisher fixture 常量
// ---------------------------------------------------------------------------

/// 标准的 docs/my-article.md 源文件内容
pub const ARTICLE_SOURCE: &str = "# My Article\n\nContent for e2e testing.";

/// 预构建的 _build/my-article.md 产物内容
pub const ARTICLE_BUILD: &str = "<h1>My Article</h1>\n<p>Content for e2e testing.</p>";

/// 含有 my-article 条目的 mind-index.yaml（供 publish 命令查找）
pub const PUBLISHER_INDEX: &str = r#"schema_version: '1'
articles:
  - title: "My Article"
    project: "my-project"
    type: blog
    source_path: "docs/my-article.md"
    status: published
    created_at: "2026-05-14T00:00:00Z"
    updated_at: "2026-05-14T00:00:00Z"
"#;

/// 有效的 repo-wide 博客发布者 blog.yaml
pub const PUBLISHER_BLOG_YAML: &str = r#"type: local
config:
  path: ./publisher-output
"#;

/// 格式错误的发布者定义 bad.yaml（非法 YAML，用于诊断测试）
pub const PUBLISHER_BAD_YAML: &str = "<<<not yaml>>>";

/// 含有 project-local publish target 的 mind.yaml（用于 fallback 测试）
/// 注意：resolve_local_path 将相对路径从 repo_root 解析，因此使用 repo 相对路径
pub const MIND_YAML_WITH_TARGETS: &str = r#"schema_version: '1'
publish:
  targets:
    - name: local-blog
      type: local
      config:
        path: ./publisher-output
"#;

impl Dataset {
    /// 在 repo 根下创建 .mind-forge/publisher/<name>.yaml 发布者定义
    pub fn with_publisher(self, name: &str, yaml: &str) -> Self {
        let dir = self.dir.path().join(".mind-forge").join("publisher");
        fs::create_dir_all(&dir).expect("create publisher dir");
        fs::write(dir.join(format!("{name}.yaml")), yaml).expect("write publisher yaml");
        self
    }

    /// 在项目下创建 source 文章 docs/<article>.md
    pub fn with_source_article(self, project: &str, article: &str, content: &str) -> Self {
        let dir = self.dir.path().join("projects").join(project).join("docs");
        fs::create_dir_all(&dir).expect("create docs dir");
        fs::write(dir.join(format!("{article}.md")), content).expect("write article");
        self
    }

    /// 在项目下创建 build 产物 _build/<article>.md
    pub fn with_build_artifact(self, project: &str, article: &str, content: &str) -> Self {
        let dir = self.dir.path().join("projects").join(project).join("_build");
        fs::create_dir_all(&dir).expect("create _build dir");
        fs::write(dir.join(format!("{article}.md")), content).expect("write build artifact");
        self
    }

    /// 创建含有完整 mind.yaml（含 publish targets）的项目
    pub fn with_project_and_targets(self, name: &str, mind_yaml: &str) -> Self {
        let dir = self.dir.path().join("projects").join(name);
        fs::create_dir_all(&dir).expect("create project dir");
        fs::write(dir.join("mind.yaml"), mind_yaml).expect("write mind.yaml");
        self
    }
}

// ---------------------------------------------------------------------------
// 预定义场景
// ---------------------------------------------------------------------------

/// 一个包含 3 个项目的 repo
pub fn repo_with_three_projects() -> Dataset {
    Dataset::empty().with_project("alpha").with_project("beta").with_project("gamma")
}

/// 一个同时包含项目和普通目录的 repo
pub fn repo_with_mixed_content() -> Dataset {
    Dataset::empty()
        .with_project("real-project")
        .with_non_project_dir("just-a-folder")
        .with_non_project_dir("another-folder")
}

// ---------------------------------------------------------------------------
// 008 Project Lifecycle 预定义验收场景
// ---------------------------------------------------------------------------

/// 含 3 个标准骨架项目（alpha/beta/gamma），各项目已注册到 minds.yaml
pub fn repo_008_empty_projects() -> Dataset {
    // 先建项目，再运行 index 注册到 minds.yaml
    let ds =
        Dataset::empty().with_standard_project("alpha").with_standard_project("beta").with_standard_project("gamma");
    // 手动注册到 minds.yaml（模拟 mf project index 的效果）
    let manifest = "schema_version: '1'\nprojects:\n  - name: alpha\n    path: ./projects/alpha\n    created_at: \"2026-04-30T08:00:00Z\"\n    archived_at: ~\n  - name: beta\n    path: ./projects/beta\n    created_at: \"2026-04-30T09:00:00Z\"\n    archived_at: ~\n  - name: gamma\n    path: ./projects/gamma\n    created_at: \"2026-04-30T10:00:00Z\"\n    archived_at: ~\n".to_string();
    fs::write(ds.dir.path().join("minds.yaml"), &manifest).expect("write manifest");
    ds
}

/// 项目含已填充的索引数据（用于 list/status 验收）
pub fn repo_008_with_data() -> Dataset {
    let ds = Dataset::empty()
        .with_standard_project("alpha")
        .with_standard_project("beta")
        .with_index("alpha", INDEX_POPULATED);
    let manifest = "schema_version: '1'\nprojects:\n  - name: alpha\n    path: ./projects/alpha\n    created_at: \"2026-04-30T08:00:00Z\"\n    archived_at: ~\n  - name: beta\n    path: ./projects/beta\n    created_at: \"2026-04-30T09:00:00Z\"\n    archived_at: ~\n".to_string();
    fs::write(ds.dir.path().join("minds.yaml"), &manifest).expect("write manifest");
    ds
}

/// 项目含过期索引条目和缺失目录（用于 lint 验收）
pub fn repo_008_with_lint_issues() -> Dataset {
    let ds = Dataset::empty()
        .with_standard_project("alpha")
        .with_index("alpha", INDEX_STALE_ENTRY) // 引用 ghost.md 但文件不存在
        .without_dir("alpha", "sources"); // 删掉 sources/
    let manifest = "schema_version: '1'\nprojects:\n  - name: alpha\n    path: ./projects/alpha\n    created_at: \"2026-04-30T08:00:00Z\"\n    archived_at: ~\n".to_string();
    fs::write(ds.dir.path().join("minds.yaml"), &manifest).expect("write manifest");
    ds
}

/// 项目含命名规范违规（用于 lint name_convention 验收）
pub fn repo_008_with_name_violation() -> Dataset {
    let ds = Dataset::empty().with_standard_project("alpha").with_index("alpha", INDEX_NAME_VIOLATION);
    let manifest = "schema_version: '1'\nprojects:\n  - name: alpha\n    path: ./projects/alpha\n    created_at: \"2026-04-30T08:00:00Z\"\n    archived_at: ~\n".to_string();
    fs::write(ds.dir.path().join("minds.yaml"), &manifest).expect("write manifest");
    ds
}

// ---------------------------------------------------------------------------
// 016 Publisher E2E 预定义验收场景
// ---------------------------------------------------------------------------

/// 包含完整 publisher 场景的 repo（有效发布者 + 无效发布者诊断 + 可发布内容）
pub fn repo_with_publishers() -> Dataset {
    Dataset::empty()
        .with_project("my-project")
        .with_publisher("blog", PUBLISHER_BLOG_YAML)
        .with_publisher("bad", PUBLISHER_BAD_YAML)
        .with_source_article("my-project", "my-article", ARTICLE_SOURCE)
        .with_build_artifact("my-project", "my-article", ARTICLE_BUILD)
        .with_index("my-project", PUBLISHER_INDEX)
}

/// 包含 project-local publish target 的 repo（用于 fallback 场景）
pub fn repo_with_project_local_target() -> Dataset {
    Dataset::empty()
        .with_project_and_targets("my-project", MIND_YAML_WITH_TARGETS)
        .with_source_article("my-project", "my-article", ARTICLE_SOURCE)
        .with_build_artifact("my-project", "my-article", ARTICLE_BUILD)
        .with_index("my-project", PUBLISHER_INDEX)
}

// ---------------------------------------------------------------------------
// 017 Mind YAML compatibility fixtures
// ---------------------------------------------------------------------------

/// Path to the compatibility testdata directory, relative to the repo root.
pub const MIND_YAML_COMPAT_TESTDATA: &str = "tests/e2e/testdata/mind-yaml-compat";

/// Copy the Mind YAML compatibility fixture directory into a temp directory.
///
/// Returns a `Dataset` pointing at the copied fixture repo. All 12 projects,
/// variant `mind.yaml` files, and dictionary `mind-index.yaml` files are
/// included from the version-controlled testdata.
pub fn mind_yaml_compat_repo() -> Dataset {
    let dir = TempDir::new().expect("temp dir");
    copy_dir_recursively(Path::new(MIND_YAML_COMPAT_TESTDATA), dir.path());
    Dataset { dir }
}

/// Recursively copy a directory tree (shallow copy, symlinks not handled).
fn copy_dir_recursively(src: &Path, dst: &Path) {
    if !src.exists() {
        panic!("source directory does not exist: {}", src.display());
    }
    for entry in std::fs::read_dir(src).expect("read source dir") {
        let entry = entry.expect("entry");
        let entry_type = entry.file_type().expect("file type");
        let src_path = entry.path();
        let rel = src_path.strip_prefix(src).expect("strip prefix");
        let dst_path = dst.join(rel);
        if entry_type.is_dir() {
            std::fs::create_dir_all(&dst_path).expect("create dir");
            copy_dir_recursively(&src_path, &dst_path);
        } else {
            std::fs::copy(&src_path, &dst_path).expect("copy file");
        }
    }
}
