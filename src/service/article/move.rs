use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::error::{MfError, Result};
use crate::service::index;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ArticleMoveReport {
    pub title: String,
    pub old_path: String,
    pub new_path: String,
    pub moved_prompts: Vec<String>,
    pub moved_thinking: Vec<String>,
    pub moved_publish_records: usize,
    pub rag_indexed: bool,
    pub dry_run: bool,
}

pub fn move_article(
    source_project: &Path,
    target_project: &Path,
    selector: &str,
    dry_run: bool,
) -> Result<ArticleMoveReport> {
    if source_project == target_project {
        return Err(MfError::usage("article source and destination projects must differ", None::<String>));
    }
    let mut source_index = index::load(source_project)?;
    let original_source_index = source_index.clone();
    let articles = source_index.articles.as_ref().ok_or_else(|| {
        MfError::not_found(
            format!("article '{selector}' not found"),
            Some("use `mf article list` to see available articles".to_string()),
        )
    })?;
    let article_pos = articles
        .iter()
        .position(|article| article.title == selector || article.article_path == selector)
        .ok_or_else(|| {
            MfError::not_found(
                format!("article '{selector}' not found"),
                Some("use `mf article list` to see available articles".to_string()),
            )
        })?;
    let article = articles[article_pos].clone();
    let old_path = article.article_path.clone();
    let new_path = old_path.clone();
    let old_full = source_project.join(&old_path);
    let new_full = target_project.join(&new_path);
    if !dry_run && !old_full.exists() {
        return Err(MfError::not_found(
            format!("article path '{}' not found", old_full.display()),
            Some("run `mf article index` to refresh the source project".to_string()),
        ));
    }

    let mut target_index = index::load(target_project)?;
    if target_index.articles.as_ref().is_some_and(|items| items.iter().any(|item| item.article_path == new_path)) {
        return Err(MfError::usage(
            format!("destination already contains article '{}'", new_path),
            Some("rename or remove the destination article before moving".to_string()),
        ));
    }
    let prompts = source_index
        .prompts
        .as_ref()
        .map(|items| items.iter().filter(|item| item.article == old_path).cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    let thinking = source_index
        .thinking
        .as_ref()
        .map(|items| items.iter().filter(|item| item.article == old_path).cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    let publish_records = source_index
        .publish_records
        .as_ref()
        .map(|items| items.iter().filter(|item| item.path == old_path).cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    for item in &prompts {
        if target_index.prompts.as_ref().is_some_and(|items| items.iter().any(|candidate| candidate.path == item.path))
        {
            return Err(MfError::usage(format!("destination already contains prompt '{}'", item.path), None::<String>));
        }
    }
    for item in &thinking {
        if target_index.thinking.as_ref().is_some_and(|items| items.iter().any(|candidate| candidate.path == item.path))
        {
            return Err(MfError::usage(
                format!("destination already contains thinking entry '{}'", item.path),
                None::<String>,
            ));
        }
    }
    let moved_prompts = prompts.iter().map(|item| item.path.clone()).collect::<Vec<_>>();
    let moved_thinking = thinking.iter().map(|item| item.path.clone()).collect::<Vec<_>>();
    let report = ArticleMoveReport {
        title: article.title.clone(),
        old_path: old_path.clone(),
        new_path: new_path.clone(),
        moved_prompts: moved_prompts.clone(),
        moved_thinking: moved_thinking.clone(),
        moved_publish_records: publish_records.len(),
        rag_indexed: false,
        dry_run,
    };
    if dry_run {
        return Ok(report);
    }
    if new_full.exists() {
        return Err(MfError::usage(
            format!("destination article path '{}' already exists", new_path),
            Some("remove the destination path before moving".to_string()),
        ));
    }
    if let Some(parent) = new_full.parent() {
        fs::create_dir_all(parent).map_err(MfError::Io)?;
    }
    fs::rename(&old_full, &new_full).map_err(MfError::Io)?;
    let mut moved_files = vec![(new_full.clone(), old_full.clone())];
    for relative in moved_prompts.iter().chain(moved_thinking.iter()) {
        let old_file = source_project.join(relative);
        let new_file = target_project.join(relative);
        if !old_file.exists() {
            continue;
        }
        if new_file.exists() {
            rollback_files(&moved_files);
            return Err(MfError::file_exists(new_file));
        }
        if let Some(parent) = new_file.parent() {
            fs::create_dir_all(parent).map_err(MfError::Io)?;
        }
        if let Err(error) = fs::rename(&old_file, &new_file) {
            rollback_files(&moved_files);
            return Err(MfError::Io(error));
        }
        moved_files.push((new_file, old_file));
    }
    source_index.articles.as_mut().unwrap().remove(article_pos);
    if let Some(items) = source_index.prompts.as_mut() {
        items.retain(|item| !moved_prompts.contains(&item.path));
    }
    if let Some(items) = source_index.thinking.as_mut() {
        items.retain(|item| !moved_thinking.contains(&item.path));
    }
    if let Some(items) = source_index.publish_records.as_mut() {
        items.retain(|item| item.path != old_path);
    }
    if let Err(error) = index::save(source_project, &source_index) {
        rollback_files(&moved_files);
        return Err(error);
    }
    target_index.articles.get_or_insert_with(Vec::new).push(article);
    target_index.prompts.get_or_insert_with(Vec::new).extend(prompts);
    target_index.thinking.get_or_insert_with(Vec::new).extend(thinking);
    target_index.publish_records.get_or_insert_with(Vec::new).extend(publish_records);
    if let Err(error) = index::save(target_project, &target_index) {
        rollback_files(&moved_files);
        let _ = index::save(source_project, &original_source_index);
        return Err(error);
    }
    Ok(report)
}

fn rollback_files(files: &[(std::path::PathBuf, std::path::PathBuf)]) {
    for (new_file, old_file) in files.iter().rev() {
        if new_file.exists() {
            let _ = fs::rename(new_file, old_file);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn moves_article_and_projection() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("source");
        let target = tmp.path().join("target");
        fs::create_dir_all(source.join("docs/post")).unwrap();
        fs::create_dir_all(source.join("prompts")).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::write(source.join("docs/post/01-main.md"), "# post\n").unwrap();
        fs::write(source.join("prompts/post.md"), "article: docs/post\n").unwrap();
        fs::write(source.join("mind-index.yaml"), "schema: '1'\narticles:\n  - title: Post\n    project: source\n    type: blank\n    article_path: docs/post\n    status: draft\n    created_at: ''\n    updated_at: ''\nprompts:\n  - path: prompts/post.md\n    article: docs/post\n").unwrap();
        let report = move_article(&source, &target, "Post", false).unwrap();
        assert_eq!(report.moved_prompts, vec!["prompts/post.md"]);
        assert!(target.join("docs/post/01-main.md").exists());
        assert!(target.join("prompts/post.md").exists());
    }
}
