use std::fs;
use std::path::Path;

use crate::error::{MfError, Result};
use crate::model::article::{ArticleIdentity, ArticleRemoveReport};
use crate::service::index;

/// Hard-remove an article: delete the file/directory and update the index.
pub fn remove_article(project_path: &Path, title: &str, force: bool, dry_run: bool) -> Result<ArticleRemoveReport> {
    crate::service::util::require_nonempty(title, "article title")?;

    let mut index = index::load(project_path)?;

    // Resolve the target via the shared selector (spec 079 US3, FR-007/009):
    // full `article_path`, `article_path` with `.md` stripped, bare slug, or
    // exact title — collecting *all* candidates and rejecting ambiguity
    // rather than guessing. Unlike the old inline matcher, this also accepts
    // a bare slug (`2026-09-monthly`, not just `docs/2026-09-monthly`).
    let matched_path = super::selector::resolve_selector(project_path, title)?;
    let articles = index.articles.as_ref().expect("resolve_selector found a match, so articles is non-empty");
    let article = articles
        .iter()
        .find(|a| a.article_path == matched_path)
        .expect("resolve_selector returned this exact article_path from this same index load");

    let scope = crate::model::lifecycle::ScopeRef { project: Some(article.project.clone()), global: false };
    let before = ArticleIdentity { title: article.title.clone(), article_path: article.article_path.clone(), scope };
    // Capture the matched entry's stable index key so deletion removes *this*
    // entity rather than re-matching the raw `title` argument (which broke when
    // the user passed the `article_path`/index-key form).
    let matched_path = article.article_path.clone();

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

    // Remove from index by the matched entry's stable key, not the raw argument.
    {
        let articles = index.articles.as_mut().expect("already checked");
        articles.retain(|a| a.article_path != matched_path);
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
