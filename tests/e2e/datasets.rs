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

    /// 在 repo 根下创建一个含 mind.yaml 的项目目录
    pub fn with_project(self, name: &str) -> Self {
        let dir = self.dir.path().join(name);
        fs::create_dir_all(&dir).expect("create project dir");
        fs::write(dir.join("mind.yaml"), MIND_YAML).expect("write mind.yaml");
        self
    }

    /// 在 repo 根下创建不含 mind.yaml 的普通目录
    pub fn with_non_project_dir(self, name: &str) -> Self {
        let dir = self.dir.path().join(name);
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

    /// 在 repo 外创建一个临时目录（用于测试非 repo 场景）
    pub fn outside() -> TempDir {
        TempDir::new().expect("temp dir")
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
