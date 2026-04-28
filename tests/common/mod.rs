use std::fs;

pub use tempfile::TempDir;

/// 创建一个临时 Mind Repo
pub fn setup_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("minds.yaml"), "schema_version: '1'\nprojects: []\n").unwrap();
    dir
}

/// 在 repo 中创建一个项目目录（含 mind.yaml）
#[allow(dead_code)]
pub fn create_project(repo: &TempDir, name: &str) {
    let project_dir = repo.path().join(name);
    fs::create_dir_all(&project_dir).unwrap();
    fs::write(project_dir.join("mind.yaml"), "schema_version: '1'\n").unwrap();
}
