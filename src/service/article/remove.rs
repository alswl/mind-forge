use std::fs;
use std::path::Path;

use crate::error::{MfError, Result};
use crate::model::article::{ArticleIdentity, ArticleRemoveReport};
use crate::service::index;

/// Hard-remove an article: delete the file/directory and update the index.
pub fn remove_article(project_path: &Path, title: &str, force: bool, dry_run: bool) -> Result<ArticleRemoveReport> {
    crate::service::util::require_nonempty(title, "article title")?;

    let mut index = index::load(project_path)?;
    let articles = index.articles.as_ref().ok_or_else(|| {
        MfError::not_found(
            format!("article '{title}' not found"),
            Some("use `mf article list --project <project>` to see available articles".to_string()),
        )
    })?;

    let article = articles.iter().find(|a| a.title == title || a.article_path == title).ok_or_else(|| {
        MfError::not_found(
            format!("article '{title}' not found"),
            Some("use `mf article list --project <project>` to see available articles".to_string()),
        )
    })?;

    let scope = crate::model::lifecycle::ScopeRef { project: Some(article.project.clone()), global: false };
    let before = ArticleIdentity { title: article.title.clone(), article_path: article.article_path.clone(), scope };

    // Reference scan (articles reference other objects, not typically referenced themselves)
    let refs: Vec<crate::model::lifecycle::Reference> = Vec::new();

    let mut planned: Vec<crate::model::lifecycle::PlannedChange> = Vec::new();
    let abs_path = project_path.join(&article.article_path);
    if abs_path.is_dir() {
        planned.push(crate::model::lifecycle::PlannedChange {
            op: crate::model::lifecycle::PlannedOp::RemoveDir,
            path: abs_path.to_string_lossy().to_string(),
            old: Some(article.title.clone()),
            new: None,
        });
    } else if abs_path.exists() {
        planned.push(crate::model::lifecycle::PlannedChange {
            op: crate::model::lifecycle::PlannedOp::RemoveFile,
            path: abs_path.to_string_lossy().to_string(),
            old: Some(article.title.clone()),
            new: None,
        });
    }
    planned.push(crate::model::lifecycle::PlannedChange {
        op: crate::model::lifecycle::PlannedOp::UpdateYaml,
        path: project_path.join("mind-index.yaml").to_string_lossy().to_string(),
        old: Some(article.title.clone()),
        new: None,
    });
    planned.push(crate::model::lifecycle::PlannedChange {
        op: crate::model::lifecycle::PlannedOp::RefreshIndex,
        path: project_path.join("mind-index.yaml").to_string_lossy().to_string(),
        old: None,
        new: None,
    });

    if dry_run {
        return Ok(ArticleRemoveReport {
            verb: "remove".into(),
            kind: "article".into(),
            before,
            after: None,
            references: refs,
            side_effects: planned,
            force,
            dry_run: true,
        });
    }

    // Remove file/directory from disk
    if abs_path.is_dir() {
        fs::remove_dir_all(&abs_path).map_err(MfError::Io)?;
    } else if abs_path.exists() {
        fs::remove_file(&abs_path).map_err(MfError::Io)?;
    }

    // Remove from index
    {
        let articles = index.articles.as_mut().expect("already checked");
        articles.retain(|a| a.title != title);
    }
    index::save(project_path, &index)?;

    Ok(ArticleRemoveReport {
        verb: "remove".into(),
        kind: "article".into(),
        before,
        after: None,
        references: refs,
        side_effects: planned,
        force,
        dry_run: false,
    })
}
