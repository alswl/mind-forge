use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::defaults;
use crate::error::{MfError, Result};
use crate::model::article::Article;
use crate::model::config::MindConfig;
use crate::service::config as config_svc;

use super::build_index;

/// List articles in a project, rebuilding the index first.
pub fn list_articles(project_path: &Path) -> Result<Vec<Article>> {
    let config = config_svc::load_project(project_path, None)?
        .ok_or_else(|| MfError::not_found("mind.yaml not found".to_string(), None))?;
    let index = build_index(project_path, &config)?;
    Ok(index.articles.unwrap_or_default())
}

/// List articles across all projects in a repo, sorted by file modification time
/// (newest first). Returns tuples of (article, project_path, mtime_seconds).
pub fn list_articles_all_projects(repo_root: &Path) -> Result<Vec<(Article, PathBuf, u64)>> {
    let projects_dir = crate::service::repo::projects_dir_for(repo_root)?;
    let manifest_path = repo_root.join("minds.yaml");
    let manifest = if manifest_path.exists() { crate::service::repo::load_manifest(&manifest_path).ok() } else { None };

    let mut project_paths: Vec<PathBuf> = Vec::new();
    if let Some(ref m) = manifest {
        for entry in &m.projects {
            project_paths.push(repo_root.join(entry.path.trim_start_matches("./")));
        }
    }
    let scanned = crate::service::repo::scan_project_dirs(repo_root, &projects_dir);
    for sp in &scanned {
        let path = repo_root.join(sp.path.trim_start_matches("./"));
        if !project_paths.contains(&path) {
            project_paths.push(path);
        }
    }

    let mut results: Vec<(Article, PathBuf, u64)> = Vec::new();
    for project_path in &project_paths {
        if let Ok(articles) = list_articles(project_path) {
            for article in articles {
                let mtime = article_file_mtime(project_path, &article.article_path);
                results.push((article, project_path.clone(), mtime));
            }
        }
    }

    results.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.article_path.cmp(&b.0.article_path)));
    Ok(results)
}

/// Get the newest file modification time (Unix epoch seconds) for an article.
pub fn article_file_mtime(project_path: &Path, article_path: &str) -> u64 {
    let full_path = project_path.join(article_path);
    match fs::metadata(&full_path) {
        Ok(meta) => {
            if meta.is_dir() {
                newest_file_mtime_in_dir(&full_path).unwrap_or(0)
            } else {
                meta_unixtime(&meta).unwrap_or(0)
            }
        }
        Err(_) => 0,
    }
}

fn newest_file_mtime_in_dir(dir: &Path) -> Option<u64> {
    let mut newest: Option<u64> = None;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Ok(meta) = fs::metadata(&path) {
                let t = if meta.is_dir() { newest_file_mtime_in_dir(&path) } else { meta_unixtime(&meta) };
                if let Some(t) = t {
                    newest = Some(newest.map_or(t, |n| n.max(t)));
                }
            }
        }
    }
    newest
}

fn meta_unixtime(meta: &fs::Metadata) -> Option<u64> {
    meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs())
}

/// Derive the article key from its article_path.
///
/// Returns the full project-relative path without `.md` extension.
/// For `"docs/my-article.md"` → `"docs/my-article"`.
/// For `"outputs/2026-05/2026-05-15.md"` → `"outputs/2026-05/2026-05-15"`.
fn article_key_from_article_path(article_path: &str) -> String {
    article_path.strip_suffix(&format!(".{}", defaults::MARKDOWN_EXTENSION)).unwrap_or(article_path).to_string()
}

/// Compute the effective article directory for an article based on project config.
///
/// Returns the project-root-relative directory path:
/// - Article's configured `article_dir` in `build.articles[article_key]` if present
/// - Otherwise `docs/<article-name>` as the default
pub fn effective_article_dir(project_path: &Path, config: &MindConfig, article: &Article) -> String {
    let article_key = article_key_from_article_path(&article.article_path);
    let layout = config_svc::effective_layout(project_path).ok();
    let articles_dir = layout.as_ref().map(|l| l.articles.as_str()).unwrap_or(defaults::DOCS_DIR);
    let docs_prefix = format!("{articles_dir}/");
    let short_key = article_key.strip_prefix(&docs_prefix).unwrap_or(&article_key);
    if let Some(article_dir) = config.build.articles.get(short_key).and_then(|a| a.article_dir.clone()) {
        return article_dir;
    }
    if let Some(article_dir) = config
        .build
        .articles
        .values()
        .filter_map(|a| a.article_dir.as_ref())
        .find(|article_dir| article_dir.as_str() == article.article_path)
    {
        return article_dir.clone();
    }
    format!("{articles_dir}/{short_key}")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── article_file_mtime tests ──

    #[test]
    fn mtime_regular_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("doc.md");
        std::fs::write(&file_path, "content").unwrap();
        let mtime = article_file_mtime(dir.path(), "doc.md");
        assert!(mtime > 0, "regular file should return non-zero mtime");
    }

    #[test]
    fn mtime_missing_path_returns_zero() {
        let dir = tempfile::tempdir().unwrap();
        let mtime = article_file_mtime(dir.path(), "nonexistent.md");
        assert_eq!(mtime, 0);
    }

    #[test]
    fn mtime_directory_returns_newest_file() {
        let dir = tempfile::tempdir().unwrap();
        let article_dir = dir.path().join("docs");
        std::fs::create_dir_all(&article_dir).unwrap();

        std::fs::write(article_dir.join("a.md"), "old").unwrap();
        std::fs::write(article_dir.join("b.md"), "newer").unwrap();

        let a_mtime = std::fs::metadata(article_dir.join("a.md"))
            .unwrap()
            .modified()
            .unwrap()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let b_mtime = std::fs::metadata(article_dir.join("b.md"))
            .unwrap()
            .modified()
            .unwrap()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let dir_mtime = article_file_mtime(dir.path(), "docs");

        let expected = a_mtime.max(b_mtime);
        assert!(dir_mtime > 0, "directory with files should return non-zero mtime");
        assert_eq!(dir_mtime, expected, "directory mtime should equal the newest file's mtime");
    }

    #[test]
    fn mtime_empty_directory_returns_zero() {
        let dir = tempfile::tempdir().unwrap();
        let empty_dir = dir.path().join("empty_article");
        std::fs::create_dir(&empty_dir).unwrap();

        let mtime = article_file_mtime(dir.path(), "empty_article");
        assert_eq!(mtime, 0);
    }
}
