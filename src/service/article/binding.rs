use std::collections::HashMap;

use serde::Serialize;

use crate::model::index::IndexFile;
use crate::model::prompt::{BindingStatus, PromptMode};
use crate::service::index;

/// The prompt binding for one article, as viewed from the article side:
/// every prompt whose `article:` frontmatter resolves to this article's
/// path. `conflicts` lists every conflicting prompt path when more than one
/// prompt binds to the same article (`binding_status: duplicate`).
#[derive(Debug, Clone, Serialize)]
pub struct ArticlePromptView {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<PromptMode>,
    pub binding_status: BindingStatus,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub conflicts: Vec<String>,
}

/// The thinking ledger association for one article, resolved by key
/// alignment. Presence alone means the ledger is bound to this article — no
/// separate `binding_status` is needed at this granularity.
#[derive(Debug, Clone, Serialize)]
pub struct ArticleThinkingView {
    pub path: String,
    pub updated_at: String,
}

/// Resolve the prompt bound to `article_path`, `None` when no prompt binds
/// to it. When multiple prompts resolve to the same article (duplicate
/// binding), the lexicographically first path is primary and every
/// conflicting path is listed in `conflicts`.
pub fn prompt_view_for_article(idx: &IndexFile, article_path: &str) -> Option<ArticlePromptView> {
    let bindings = index::resolve_prompt_bindings(idx);
    let mut matches: Vec<&index::PromptBinding<'_>> =
        bindings.iter().filter(|b| b.prompt.article == article_path).collect();
    if matches.is_empty() {
        return None;
    }
    matches.sort_by(|a, b| a.prompt.path.cmp(&b.prompt.path));

    let primary = matches[0];
    let conflicts: Vec<String> =
        if matches.len() > 1 { matches.iter().map(|b| b.prompt.path.clone()).collect() } else { Vec::new() };

    Some(ArticlePromptView {
        path: primary.prompt.path.clone(),
        mode: primary.prompt.mode,
        binding_status: primary.status,
        updated_at: primary.prompt.updated_at.clone(),
        conflicts,
    })
}

/// Resolve the thinking ledger entry associated with `article_path` by key
/// alignment (own filename stem). `None` when no thinking file shares the
/// article's key.
pub fn thinking_view_for_article(idx: &IndexFile, article_path: &str) -> Option<ArticleThinkingView> {
    let key = index::article_output_stem(article_path);
    idx.thinking
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .find(|t| index::derive_store_key(&t.path) == key)
        .map(|t| ArticleThinkingView { path: t.path.clone(), updated_at: t.updated_at.clone() })
}

/// Resolve every article's prompt view in a single pass, keyed by article
/// path. Equivalent to calling [`prompt_view_for_article`] once per article,
/// but O(prompts) total instead of O(prompts) per article — use this from a
/// loop over all of a project's articles (e.g. `article list`).
pub fn prompt_views_by_article(idx: &IndexFile) -> HashMap<String, ArticlePromptView> {
    let bindings = index::resolve_prompt_bindings(idx);
    let mut by_article: HashMap<&str, Vec<&index::PromptBinding<'_>>> = HashMap::new();
    for b in &bindings {
        if !b.prompt.article.is_empty() {
            by_article.entry(b.prompt.article.as_str()).or_default().push(b);
        }
    }

    by_article
        .into_iter()
        .map(|(article, mut matches)| {
            matches.sort_by(|a, b| a.prompt.path.cmp(&b.prompt.path));
            let primary = matches[0];
            let conflicts: Vec<String> =
                if matches.len() > 1 { matches.iter().map(|b| b.prompt.path.clone()).collect() } else { Vec::new() };
            (
                article.to_string(),
                ArticlePromptView {
                    path: primary.prompt.path.clone(),
                    mode: primary.prompt.mode,
                    binding_status: primary.status,
                    updated_at: primary.prompt.updated_at.clone(),
                    conflicts,
                },
            )
        })
        .collect()
}

/// Resolve every thinking entry's view in a single pass, keyed by its own
/// store key (see [`thinking_view_for_article`] for the key-alignment
/// lookup). Use this from a loop over all of a project's articles, looking
/// up each article's view via `map.get(index::article_output_stem(path))`.
pub fn thinking_views_by_key(idx: &IndexFile) -> HashMap<String, ArticleThinkingView> {
    idx.thinking
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|t| {
            (
                index::derive_store_key(&t.path),
                ArticleThinkingView { path: t.path.clone(), updated_at: t.updated_at.clone() },
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::article::{Article, ArticleStatus, ArticleType};
    use crate::model::prompt::Prompt;
    use crate::model::thinking::Thinking;

    fn article(path: &str) -> Article {
        Article {
            title: "T".to_string(),
            project: "alpha".to_string(),
            article_type: ArticleType::Blog,
            article_path: path.to_string(),
            status: ArticleStatus::Draft,
            created_at: "2026-07-01T00:00:00Z".to_string(),
            updated_at: "2026-07-01T00:00:00Z".to_string(),
            template_origin: None,
        }
    }

    #[test]
    fn prompt_view_none_when_no_prompt_binds() {
        let idx = IndexFile { articles: Some(vec![article("docs/a.md")]), ..IndexFile::create_default() };
        assert!(prompt_view_for_article(&idx, "docs/a.md").is_none());
    }

    #[test]
    fn prompt_view_bound_single_match() {
        let idx = IndexFile {
            articles: Some(vec![article("docs/a.md")]),
            prompts: Some(vec![Prompt {
                path: "prompts/a.md".to_string(),
                article: "docs/a.md".to_string(),
                mode: Some(PromptMode::Research),
                updated_at: "2026-07-12T00:00:00Z".to_string(),
            }]),
            ..IndexFile::create_default()
        };
        let view = prompt_view_for_article(&idx, "docs/a.md").expect("prompt present");
        assert_eq!(view.path, "prompts/a.md");
        assert_eq!(view.binding_status, BindingStatus::Bound);
        assert!(view.conflicts.is_empty());
    }

    #[test]
    fn prompt_view_duplicate_lists_all_conflicts() {
        let idx = IndexFile {
            articles: Some(vec![article("docs/a.md")]),
            prompts: Some(vec![
                Prompt {
                    path: "prompts/b.md".to_string(),
                    article: "docs/a.md".to_string(),
                    mode: None,
                    updated_at: "2026-07-12T00:00:00Z".to_string(),
                },
                Prompt {
                    path: "prompts/a.md".to_string(),
                    article: "docs/a.md".to_string(),
                    mode: None,
                    updated_at: "2026-07-12T00:00:00Z".to_string(),
                },
            ]),
            ..IndexFile::create_default()
        };
        let view = prompt_view_for_article(&idx, "docs/a.md").expect("prompt present");
        assert_eq!(view.binding_status, BindingStatus::Duplicate);
        assert_eq!(view.conflicts, vec!["prompts/a.md".to_string(), "prompts/b.md".to_string()]);
    }

    #[test]
    fn thinking_view_present_and_absent() {
        let idx = IndexFile {
            articles: Some(vec![article("docs/a.md")]),
            thinking: Some(vec![Thinking {
                path: "thinking/a.md".to_string(),
                article: "docs/a.md".to_string(),
                updated_at: "2026-07-12T00:00:00Z".to_string(),
            }]),
            ..IndexFile::create_default()
        };
        assert!(thinking_view_for_article(&idx, "docs/a.md").is_some());
        assert!(thinking_view_for_article(&idx, "docs/missing.md").is_none());
    }

    #[test]
    fn batch_prompt_views_match_per_article_lookup() {
        let idx = IndexFile {
            articles: Some(vec![article("docs/a.md"), article("docs/b.md")]),
            prompts: Some(vec![
                Prompt {
                    path: "prompts/a.md".to_string(),
                    article: "docs/a.md".to_string(),
                    mode: Some(PromptMode::Research),
                    updated_at: "2026-07-12T00:00:00Z".to_string(),
                },
                Prompt {
                    path: "prompts/b.md".to_string(),
                    article: "docs/b.md".to_string(),
                    mode: None,
                    updated_at: "2026-07-12T00:00:00Z".to_string(),
                },
                Prompt {
                    path: "prompts/b-dup.md".to_string(),
                    article: "docs/b.md".to_string(),
                    mode: None,
                    updated_at: "2026-07-12T00:00:00Z".to_string(),
                },
            ]),
            ..IndexFile::create_default()
        };
        let batch = prompt_views_by_article(&idx);
        assert_eq!(batch.len(), 2);
        let a_view = batch.get("docs/a.md").expect("a bound");
        assert_eq!(a_view.binding_status, BindingStatus::Bound);
        assert_eq!(a_view.path, "prompts/a.md");
        let b_view = batch.get("docs/b.md").expect("b duplicate");
        assert_eq!(b_view.binding_status, BindingStatus::Duplicate);
        assert_eq!(b_view.conflicts, vec!["prompts/b-dup.md".to_string(), "prompts/b.md".to_string()]);

        // Batch result must agree with the per-article lookup for every article.
        assert_eq!(a_view.path, prompt_view_for_article(&idx, "docs/a.md").unwrap().path);
        assert_eq!(b_view.conflicts, prompt_view_for_article(&idx, "docs/b.md").unwrap().conflicts);
    }

    #[test]
    fn batch_thinking_views_match_per_article_lookup() {
        let idx = IndexFile {
            articles: Some(vec![article("docs/a.md")]),
            thinking: Some(vec![Thinking {
                path: "thinking/a.md".to_string(),
                article: "docs/a.md".to_string(),
                updated_at: "2026-07-12T00:00:00Z".to_string(),
            }]),
            ..IndexFile::create_default()
        };
        let batch = thinking_views_by_key(&idx);
        assert_eq!(batch.len(), 1);
        let key = index::article_output_stem("docs/a.md");
        assert_eq!(batch.get(key).unwrap().path, "thinking/a.md");
        assert!(thinking_view_for_article(&idx, "docs/a.md").is_some());
    }
}
