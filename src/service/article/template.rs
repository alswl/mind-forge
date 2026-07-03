use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::error::{MfError, Result};
use crate::model::article::ArticleType;
use crate::service::util;

#[allow(dead_code)]
const ARTICLE_TEMPLATE: &str = r#"# {title}

> Created: {created_at}

## Summary

## Content
"#;

const TEMPLATE_BLANK: &str = "# {title}\n\n> Created: {created_at}\n";
const TEMPLATE_ARCH: &str = "# {title}\n\n> Created: {created_at}\n\n## Context\n\n## Decision\n\n## Consequence\n\n## Alternatives Considered\n";
const TEMPLATE_PRD: &str =
    "# {title}\n\n> Created: {created_at}\n\n## Background\n\n## Goals\n\n## Non-Goals\n\n## Requirements\n";
const TEMPLATE_BLOG: &str = "# {title}\n\n> Created: {created_at}\n\n## Summary\n\n## Content\n";

pub(crate) fn builtin_template(name: &str) -> Option<(&'static str, ArticleType)> {
    match name {
        "blank" => Some((TEMPLATE_BLANK, ArticleType::Blank)),
        "arch" => Some((TEMPLATE_ARCH, ArticleType::Arch)),
        "prd" => Some((TEMPLATE_PRD, ArticleType::Prd)),
        "blog" => Some((TEMPLATE_BLOG, ArticleType::Blog)),
        _ => None,
    }
}

pub(crate) fn resolve_custom_template_path(project_path: &Path, template_arg: &str) -> Result<PathBuf> {
    let relative = Path::new(template_arg);
    if relative.components().any(|c| matches!(c, Component::ParentDir | Component::RootDir | Component::Prefix(_))) {
        return Err(MfError::usage(
            format!("template path '{template_arg}' is outside the project root"),
            Some("use a path relative to the project root".to_string()),
        ));
    }

    let tmpl_path = project_path.join(relative);
    if tmpl_path.exists() {
        return util::canonicalize_within(project_path, &tmpl_path);
    }

    if let Some(parent) = tmpl_path.parent() {
        if parent.exists() {
            let _ = util::canonicalize_within(project_path, parent)?;
        }
    }
    Ok(tmpl_path)
}

/// Resolve a template argument into the body string.
///
/// Checks builtin templates first, then falls back to a project-root-relative
/// file lookup. Used by both the real and dry-run article creation paths.
pub fn resolve_template(project_path: &Path, template_arg: &str, title: &str) -> Result<String> {
    if let Some((body, _at)) = builtin_template(template_arg) {
        return Ok(body.replace("{title}", title));
    }
    let tmpl_path = resolve_custom_template_path(project_path, template_arg)?;
    let body = fs::read_to_string(&tmpl_path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            MfError::UnknownTemplate { name: template_arg.to_string() }
        } else {
            MfError::Io(e)
        }
    })?;
    Ok(body.replace("{title}", title))
}

/// Validate that a resolved template body parses without duplicate block slugs.
///
/// Runs `split_template_into_blocks` and discards the result on success.
/// Returns [`MfError::DuplicateBlockSlug`] when validation fails.
pub fn validate_template_blocks(resolved: &str) -> Result<()> {
    split_template_into_blocks(resolved)?;
    Ok(())
}

/// Split a resolved template body into block files for a directory article.
///
/// LF-normalises input, scans for `^## ` headings, and returns a vector of
/// `(filename, body)` pairs. Returns [`MfError::DuplicateBlockSlug`] when
/// two headings produce the same slug.
pub(crate) fn split_template_into_blocks(resolved: &str) -> Result<Vec<(String, String)>> {
    let normalized = resolved.replace("\r\n", "\n");
    let lines: Vec<&str> = normalized.lines().collect();

    struct Block {
        h2_text: String,
        slug: String,
        body: String,
    }

    let mut raw: Vec<Block> = Vec::new();
    let mut head_lines: Vec<&str> = Vec::new();
    let mut current_h2: Option<&str> = None;
    let mut current_body: Vec<&str> = Vec::new();
    let mut fence_tracker = crate::service::util::markdown::FenceTracker::new();

    for line in &lines {
        let inside_fence =
            matches!(fence_tracker.process_line(line), crate::service::util::markdown::FenceStatus::Inside);
        // `## ` headings inside fenced code blocks are body text, not block
        // boundaries (Bug #10 fix) — an in-fence heading falls through to the
        // ordinary line branches below.
        if let Some(h2_text) = line.strip_prefix("## ").filter(|_| !inside_fence) {
            // Real heading outside a fence — close current block (if any)
            // and start a new one.
            if let Some(prev_h2) = current_h2.take() {
                let slug = util::to_filename(prev_h2.trim());
                let body = current_body.join("\n");
                raw.push(Block { h2_text: prev_h2.to_string(), slug, body });
            } else {
                let body = head_lines.join("\n");
                raw.push(Block { h2_text: String::new(), slug: String::new(), body });
                head_lines.clear();
            }
            current_h2 = Some(h2_text);
            current_body = vec![*line];
        } else if current_h2.is_some() {
            current_body.push(line);
        } else {
            head_lines.push(line);
        }
    }

    if let Some(h2_text) = current_h2.take() {
        let slug = util::to_filename(h2_text.trim());
        let body = current_body.join("\n");
        raw.push(Block { h2_text: h2_text.to_string(), slug, body });
    } else {
        let body = head_lines.join("\n");
        raw.push(Block { h2_text: String::new(), slug: String::new(), body });
    }

    // Check for duplicate slugs among H2 blocks (skip head block at index 0)
    let mut slug_map: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    for block in raw.iter().skip(1) {
        if block.slug.is_empty() {
            continue;
        }
        if let Some(prev_h2) = slug_map.insert(&block.slug, &block.h2_text) {
            return Err(MfError::DuplicateBlockSlug {
                slug: block.slug.clone(),
                h1: prev_h2.to_string(),
                h2: block.h2_text.clone(),
            });
        }
    }

    let mut result: Vec<(String, String)> = Vec::new();
    for (i, block) in raw.into_iter().enumerate() {
        if i == 0 {
            result.push(("01-opening.md".to_string(), block.body));
        } else {
            result.push((format!("{:02}-{}.md", i + 1, block.slug), block.body));
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── T007: builtin_template tests ──

    #[test]
    fn builtin_blank() {
        let (body, at) = builtin_template("blank").unwrap();
        assert_eq!(at, ArticleType::Blank);
        assert!(body.contains("{title}"));
    }

    #[test]
    fn builtin_arch() {
        let (body, at) = builtin_template("arch").unwrap();
        assert_eq!(at, ArticleType::Arch);
        assert!(body.contains("## Context"));
        assert!(body.contains("## Decision"));
        assert!(body.contains("## Consequence"));
        assert!(body.contains("## Alternatives Considered"));
    }

    #[test]
    fn builtin_prd() {
        let (body, at) = builtin_template("prd").unwrap();
        assert_eq!(at, ArticleType::Prd);
        assert!(body.contains("## Background"));
        assert!(body.contains("## Goals"));
    }

    #[test]
    fn builtin_blog_matches_legacy_article_template() {
        let (body, at) = builtin_template("blog").unwrap();
        assert_eq!(at, ArticleType::Blog);
        assert_eq!(body, ARTICLE_TEMPLATE, "TEMPLATE_BLOG must be byte-identical to ARTICLE_TEMPLATE");
    }

    #[test]
    fn builtin_blog_body_matches_legacy_byte_for_byte() {
        let (body, _) = builtin_template("blog").unwrap();
        assert_eq!(body, super::ARTICLE_TEMPLATE);
    }

    #[test]
    fn builtin_unknown_returns_none() {
        assert!(builtin_template("nope").is_none());
        assert!(builtin_template("ARCH").is_none());
        assert!(builtin_template("").is_none());
    }

    #[test]
    fn builtin_h2_slugs_are_distinct() {
        for name in &["arch", "prd", "blog"] {
            let (body, _) = builtin_template(name).unwrap();
            let mut slugs: std::collections::HashSet<String> = std::collections::HashSet::new();
            for line in body.lines() {
                if let Some(h2) = line.strip_prefix("## ") {
                    let slug = util::to_filename(h2.trim());
                    assert!(slugs.insert(slug.clone()), "duplicate slug '{slug}' in template '{name}'");
                }
            }
        }
    }

    // ── T008: split_template_into_blocks tests ──

    #[test]
    fn split_zero_h2_produces_head_only() {
        let input = "# Title\n\nBody text\n";
        let blocks = split_template_into_blocks(input).unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].0, "01-opening.md");
        assert_eq!(blocks[0].1, "# Title\n\nBody text");
    }

    #[test]
    fn split_four_h2_arch_style() {
        let input = "# Title\n\n> Created: now\n\n## Context\n\nctx body\n\n## Decision\n\ndec\n\n## Consequence\n\ncons\n\n## Alternatives Considered\n\nalt\n";
        let blocks = split_template_into_blocks(input).unwrap();
        assert_eq!(blocks.len(), 5);
        assert_eq!(blocks[0].0, "01-opening.md");
        assert!(blocks[0].1.starts_with("# Title"));
        assert_eq!(blocks[1].0, "02-context.md");
        assert!(blocks[1].1.starts_with("## Context"));
        assert_eq!(blocks[2].0, "03-decision.md");
        assert_eq!(blocks[3].0, "04-consequence.md");
        assert_eq!(blocks[4].0, "05-alternatives-considered.md");
    }

    #[test]
    fn split_leading_h2_empty_head() {
        let input = "## Intro\n\nintro body\n";
        let blocks = split_template_into_blocks(input).unwrap();
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].0, "01-opening.md");
        assert_eq!(blocks[0].1, "");
        assert_eq!(blocks[1].0, "02-intro.md");
        assert_eq!(blocks[1].1, "## Intro\n\nintro body");
    }

    #[test]
    fn split_crlf_normalized() {
        let input = "# Title\r\n\r\n## Summary\r\n\r\nbody\r\n";
        let blocks = split_template_into_blocks(input).unwrap();
        assert_eq!(blocks.len(), 2);
        assert!(!blocks[0].1.contains('\r'));
        assert!(!blocks[1].1.contains('\r'));
    }

    #[test]
    fn split_duplicate_slug_rejected() {
        let input = "# T\n\n## Notes\n\nbody1\n\n## NOTES\n\nbody2\n";
        let err = split_template_into_blocks(input).unwrap_err();
        assert!(matches!(err, MfError::DuplicateBlockSlug { .. }));
        assert_eq!(err.kind(), "duplicate_block_slug");
    }

    #[test]
    fn split_single_h2() {
        let input = "# Title\n\nintro\n\n## Summary\n\nsummary body\n";
        let blocks = split_template_into_blocks(input).unwrap();
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].0, "01-opening.md");
        assert_eq!(blocks[0].1, "# Title\n\nintro\n");
        assert_eq!(blocks[1].0, "02-summary.md");
        assert!(blocks[1].1.starts_with("## Summary"));
    }

    #[test]
    fn split_ignores_h2_inside_fenced_code_block() {
        // Bug #10: a `## ` heading inside a fence is body text, not a block
        // boundary. Only the two real headings start blocks.
        let input = "# T\n\n## Real One\n\n```md\n## Not A Heading\nmore fenced\n```\n\ntext\n\n## Real Two\n\nbody\n";
        let blocks = split_template_into_blocks(input).unwrap();
        assert_eq!(blocks.len(), 3, "head + two real headings only: {blocks:?}");
        assert_eq!(blocks[1].0, "02-real-one.md");
        assert!(blocks[1].1.contains("## Not A Heading"), "fenced heading stays in body: {:?}", blocks[1].1);
        assert_eq!(blocks[2].0, "03-real-two.md");
    }

    #[test]
    fn split_cjk_headings_produce_distinct_slugs() {
        // Bug #10 residual: CJK headings must not all collapse to "untitled".
        let input = "# 标题\n\n## 本周进展\n\na\n\n## 里程碑规划与进展\n\nb\n";
        let blocks = split_template_into_blocks(input).unwrap();
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[1].0, "02-本周进展.md");
        assert_eq!(blocks[2].0, "03-里程碑规划与进展.md");
    }
}
