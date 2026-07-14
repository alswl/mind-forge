use std::fs;
use std::path::Path;

use chrono::Utc;
use serde::Serialize;

use crate::error::{MfError, Result};
use crate::model::article::{Article, ArticleStatus};
use crate::service::index;

/// Replace the `title:` binding in a prompt's YAML frontmatter.
fn update_prompt_title(content: &str, new_title: &str) -> String {
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let mut in_frontmatter = false;
    let mut frontmatter_open = false;
    for line in &mut lines {
        if line.trim() == "---" {
            if !frontmatter_open {
                frontmatter_open = true;
                in_frontmatter = true;
                continue;
            } else if in_frontmatter {
                break;
            }
        }
        if in_frontmatter && let Some(_rest) = line.strip_prefix("title:") {
            *line = format!("title: {}", new_title);
        }
    }
    lines.join("\n") + if content.ends_with('\n') { "\n" } else { "" }
}

#[derive(Debug, Clone)]
pub struct ArticleUpdate<'a> {
    pub selector: &'a str,
    pub status: Option<ArticleStatus>,
    pub title: Option<&'a str>,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ArticleUpdateReport {
    pub article: Article,
    pub dry_run: bool,
    pub changes: serde_json::Value,
}

pub fn update_article(project_path: &Path, update: ArticleUpdate<'_>) -> Result<ArticleUpdateReport> {
    if update.status.is_none() && update.title.is_none() {
        return Err(MfError::usage(
            "nothing to update: use --status and/or --title",
            Some("pass --status draft|published or --title \"New Title\"".to_string()),
        ));
    }

    let mut index_file = index::load(project_path)?;
    let articles = index_file.articles.as_mut().ok_or_else(|| article_not_found(update.selector))?;
    let article = articles
        .iter_mut()
        .find(|a| a.title == update.selector || a.article_path == update.selector)
        .ok_or_else(|| article_not_found(update.selector))?;

    let mut changes = serde_json::Map::new();
    if let Some(status) = update.status.clone() {
        changes.insert("status".to_string(), serde_json::json!({"from": article.status, "to": status}));
        if !update.dry_run {
            article.status = status;
            article.updated_at = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        }
    }
    if let Some(title) = update.title {
        let trimmed = title.trim();
        if trimmed.is_empty() {
            return Err(MfError::usage("title must not be empty", Some("pass a non-empty title".to_string())));
        }
        changes.insert("title".to_string(), serde_json::json!({"from": article.title, "to": trimmed}));
        if !update.dry_run {
            article.title = trimmed.to_string();
            article.updated_at = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

            // Update title in associated prompt frontmatter if it exists
            let key = crate::service::index::article_output_stem(&article.article_path);
            let prompt_path = project_path.join("prompts").join(format!("{key}.md"));
            if prompt_path.exists()
                && let Ok(content) = fs::read_to_string(&prompt_path)
            {
                let updated = update_prompt_title(&content, trimmed);
                let _ = fs::write(&prompt_path, updated);
            }
        }
    }

    let updated = article.clone();
    if !update.dry_run {
        index::save(project_path, &index_file)?;
    }

    Ok(ArticleUpdateReport { article: updated, dry_run: update.dry_run, changes: serde_json::Value::Object(changes) })
}

fn article_not_found(selector: &str) -> MfError {
    MfError::not_found(
        format!("article '{selector}' not found"),
        Some("use `mf article list --project <project>` to see available articles".to_string()),
    )
}
