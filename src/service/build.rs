use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::defaults;
use crate::error::{MfError, Result};
use crate::model::config::{BannerConfig, BannerLevel};
use crate::service::{config as config_svc, index, util};

/// A single article file entry in a build plan.
#[derive(Debug, Serialize)]
pub struct ArticleInputEntry {
    pub path: String,
    pub size: u64,
}

/// Information about the configured build banner, included in dry-run output.
#[derive(Debug, Serialize)]
pub struct BannerInfo {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// A build plan describing what would happen during a build.
#[derive(Debug, Serialize)]
pub struct BuildPlan {
    pub article: String,
    pub project: String,
    pub article_path: String,
    pub input_files: Vec<ArticleInputEntry>,
    pub merge_order: Vec<String>,
    pub output_path: String,
    pub size_bytes: u64,
    pub estimated_size: u64,
    pub dry_run: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub banner: Option<BannerInfo>,
}

/// Render banner config into the Markdown text to insert into generated output.
pub fn render_banner(banner: &BannerConfig) -> String {
    match &banner.level {
        Some(level) => {
            let level_str = match level {
                BannerLevel::Note => "note",
                BannerLevel::Tip => "tip",
                BannerLevel::Warning => "warning",
                BannerLevel::Danger => "danger",
            };
            let text = banner.text.trim();
            format!(":::{level_str}\n{text}\n:::\n\n")
        }
        None => {
            let text = banner.text.trim();
            format!("{text}\n\n")
        }
    }
}

/// Insert banner text into generated content after a frontmatter header if present,
/// otherwise at the very beginning.
fn insert_banner_into_content(content: &str, banner_text: &str) -> String {
    // Check for YAML frontmatter delimited by `---`
    if let Some(rest) = content.strip_prefix("---") {
        // Find the closing `---` marker
        if let Some(end) = rest.find("\n---") {
            let split = 3 + end + 4; // position after closing `---`
                                     // Check if there's a newline after the closing `---`
            let after_split =
                if split < content.len() && content[split..].starts_with('\n') { split + 1 } else { split };
            let mut result = content[..after_split].to_string();
            result.push('\n');
            result.push_str(banner_text);
            result.push_str(&content[after_split..]);
            return result;
        }
    }
    // No frontmatter: prepend banner
    let mut result = banner_text.to_string();
    result.push_str(content);
    result
}

/// Remove Typora-only metadata from generated build content.
///
/// Source files keep the `typora-copy-images-to` key for editor convenience,
/// but build artifacts should not publish that local editor setting.
fn strip_typora_front_matter(content: &str) -> String {
    if let Some((front, body, eol)) = split_initial_yaml_front_matter(content) {
        let mut kept = String::new();
        let mut removed = false;

        for line in front.split_inclusive('\n') {
            if is_typora_copy_images_to_line(line) {
                removed = true;
            } else {
                kept.push_str(line);
            }
        }

        if !removed {
            return content.to_string();
        }

        if kept.lines().any(|line| !line.trim().is_empty()) {
            let mut result = String::new();
            result.push_str("---");
            result.push_str(eol);
            result.push_str(&kept);
            if !kept.is_empty() && !kept.ends_with('\n') {
                result.push_str(eol);
            }
            result.push_str("---");
            result.push_str(eol);
            result.push_str(body);
            return result;
        }

        return body.strip_prefix(eol).unwrap_or(body).to_string();
    }

    content.to_string()
}

fn split_initial_yaml_front_matter(content: &str) -> Option<(&str, &str, &'static str)> {
    let (opening_len, eol) = if content.starts_with("---\r\n") {
        (5, "\r\n")
    } else if content.starts_with("---\n") {
        (4, "\n")
    } else {
        return None;
    };

    let remaining = &content[opening_len..];
    let mut offset = 0;
    for line in remaining.split_inclusive('\n') {
        let line_body = line.trim_end_matches(['\r', '\n']);
        let next_offset = offset + line.len();
        if line_body == "---" {
            let front = &remaining[..offset];
            let body = &remaining[next_offset..];
            return Some((front, body, eol));
        }
        offset = next_offset;
    }

    let trailing = &remaining[offset..];
    if trailing == "---" {
        let front = &remaining[..offset];
        return Some((front, "", eol));
    }

    None
}

fn is_typora_copy_images_to_line(line: &str) -> bool {
    let line = line.trim_start().trim_end_matches(['\r', '\n']);
    line.starts_with("typora-copy-images-to:")
}

/// Build an article: load config, resolve sources, render to output.
///
/// `article` is the article name (filename stem within `docs/`).
/// `project_path` is the resolved project directory.
/// `repo_root` is the mind-forge repository root for config resolution.
/// `dry_run` controls whether to produce a plan or actual output.
/// `output_override` optionally overrides the default output path.
pub fn build_article(
    project_path: &Path,
    repo_root: &Path,
    article: &str,
    dry_run: bool,
    output_override: Option<&Path>,
) -> Result<BuildOutput> {
    // Check for configured article_dir in build.articles.<article>.article_dir
    let article_path = match config_svc::load_project(project_path, Some(repo_root)) {
        Ok(Some(config)) => config
            .build
            .articles
            .get(article)
            .and_then(|a| a.article_dir.as_ref())
            .map(|dir| {
                let p = project_path.join(dir);
                if !p.exists() || !p.is_dir() {
                    return Err(MfError::usage(
                        format!("configured article_dir '{dir}' for article '{article}' does not exist or is not a directory"),
                        Some("check the path or create the directory".to_string()),
                    ));
                }
                Ok(p)
            })
            .transpose()?,
        _ => None,
    };

    let article_path = match article_path {
        Some(path) => path,
        None => resolve_indexed_article_path(project_path, article)?,
    };
    build_article_content(project_path, repo_root, article, &article_path, dry_run, output_override)
}

pub fn build_article_path(
    project_path: &Path,
    repo_root: &Path,
    article_path: &Path,
    dry_run: bool,
    output_override: Option<&Path>,
) -> Result<BuildOutput> {
    let article = article_path
        .file_stem()
        .or_else(|| article_path.file_name())
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "article".to_string());
    build_article_content(project_path, repo_root, &article, article_path, dry_run, output_override)
}

fn resolve_indexed_article_path(project_path: &Path, article: &str) -> Result<PathBuf> {
    let index = index::load(project_path)?;
    let resolved = index::resolve_article(&index, article)?;
    let article_path = project_path.join(&resolved.article.article_path);
    if article_path.exists() {
        return Ok(article_path);
    }
    Err(MfError::usage(
        format!("article path not found: {}", resolved.article.article_path),
        Some("the file or directory may have been moved or deleted".to_string()),
    ))
}

fn build_article_content(
    project_path: &Path,
    repo_root: &Path,
    article: &str,
    article_path: &Path,
    dry_run: bool,
    output_override: Option<&Path>,
) -> Result<BuildOutput> {
    // 1. Load project config (mind.yaml)
    let config = config_svc::load_project(project_path, Some(repo_root))?.ok_or_else(|| {
        MfError::usage("project missing mind.yaml".to_string(), Some("run `mf config init` to create one".to_string()))
    })?;
    let build_cfg = &config.build;

    // 1b. Validate new config fields (e.g. empty banner text)
    config_svc::validate_new_fields(&config)?;

    // 2. Render banner if configured
    let banner_text: Option<String> = build_cfg.banner.as_ref().map(render_banner);
    let banner_size: u64 = banner_text.as_ref().map(|t| t.len() as u64).unwrap_or(0);

    // 3. Build banner info for dry-run output
    let banner_info: Option<BannerInfo> = build_cfg.banner.as_ref().map(|b| {
        let level_str = b.level.as_ref().map(|l| {
            match l {
                BannerLevel::Note => "note",
                BannerLevel::Tip => "tip",
                BannerLevel::Warning => "warning",
                BannerLevel::Danger => "danger",
            }
            .to_string()
        });
        BannerInfo { enabled: true, level: level_str, text: Some(b.text.clone()) }
    });

    if !article_path.exists() {
        return Err(MfError::usage(
            format!("article path not found: {}", article_path.display()),
            Some("the file or directory may have been moved or deleted".to_string()),
        ));
    }

    // 4. Determine output path
    let layout = config_svc::effective_layout(project_path)?;
    let output_path = match output_override {
        Some(path) => path.to_path_buf(),
        None => {
            let stem = crate::service::index::article_output_stem(article);
            project_path.join(&layout.build_output).join(format!("{stem}.{}", build_cfg.format))
        }
    };

    // 5. Gather article files — single file or directory contents
    let article_files: Vec<std::path::PathBuf> = if article_path.is_dir() {
        let mut files: Vec<_> = fs::read_dir(article_path)
            .map_err(MfError::Io)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == defaults::MARKDOWN_EXTENSION))
            .map(|e| e.path())
            .collect();
        files.sort();
        files
    } else {
        vec![article_path.to_path_buf()]
    };

    if article_files.is_empty() {
        return Err(MfError::usage(
            format!("no markdown files found in article directory: {}", article_path.display()),
            None,
        ));
    }

    let total_size: u64 = article_files.iter().filter_map(|p| fs::metadata(p).ok()).map(|m| m.len()).sum();

    // 6. Determine merge order (from config or fallback)
    let merge_order =
        if build_cfg.merge_order.is_empty() { vec![article.to_string()] } else { build_cfg.merge_order.clone() };

    // 7. Build plan output (for dry-run)
    let input_files: Vec<ArticleInputEntry> = article_files
        .iter()
        .map(|p| ArticleInputEntry {
            path: p.strip_prefix(project_path).unwrap_or(p).to_string_lossy().to_string(),
            size: fs::metadata(p).ok().map(|m| m.len()).unwrap_or(0),
        })
        .collect();

    if dry_run {
        return Ok(BuildOutput::Plan(BuildPlan {
            article: article.to_string(),
            project: util::dir_name(project_path),
            article_path: article_path.to_string_lossy().to_string(),
            input_files,
            merge_order,
            output_path: output_path.to_string_lossy().to_string(),
            size_bytes: total_size,
            estimated_size: total_size + banner_size,
            dry_run: true,
            banner: banner_info,
        }));
    }

    // 8. Read and concatenate files
    let mut content = String::new();
    for file in &article_files {
        let file_content = fs::read_to_string(file).map_err(MfError::Io)?;
        let file_content = strip_typora_front_matter(&file_content);
        content.push_str(&file_content);
        if !content.ends_with('\n') {
            content.push('\n');
        }
    }

    // 8b. Rewrite relative paths to resolve from the output directory (Bug #1 fix).
    let source_dir = if article_path.is_dir() {
        article_path.to_path_buf()
    } else {
        article_path.parent().unwrap_or(Path::new(".")).to_path_buf()
    };
    let output_dir = output_path.parent().unwrap_or(Path::new("."));
    content = rewrite_relative_paths(&content, &source_dir, output_dir);

    // 9. Inject banner into content if configured
    if let Some(ref banner) = banner_text {
        content = insert_banner_into_content(&content, banner);
    }

    // Ensure output directory exists
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(MfError::Io)?;
    }

    util::atomic_write(&output_path, &content)?;
    let result =
        BuildResult { output_path: output_path.to_string_lossy().to_string(), size_bytes: content.len() as u64 };

    // Auto-index: ensure the built article exists in mind-index.yaml
    if let Err(e) = auto_index_article(project_path, article, article_path) {
        tracing::warn!("failed to auto-index article '{}': {}", article, e);
    }

    Ok(BuildOutput::Rendered(result))
}

/// Result of a successful build.
#[derive(Debug)]
pub struct BuildResult {
    pub output_path: String,
    pub size_bytes: u64,
}

/// Build output: either the rendered content or a dry-run plan.
#[derive(Debug)]
pub enum BuildOutput {
    Rendered(BuildResult),
    Plan(BuildPlan),
}

/// Rewrite relative image/link/reference paths in `content` so they resolve
/// from `output_dir` to the same target they resolved to from `source_dir`.
/// Skips paths inside fenced code blocks, absolute paths, URLs, data URIs,
/// and anchors.
fn rewrite_relative_paths(content: &str, source_dir: &Path, output_dir: &Path) -> String {
    use crate::service::util::markdown::{FenceStatus, FenceTracker};

    let mut result = String::with_capacity(content.len());
    let mut fence = FenceTracker::new();

    for line in content.lines() {
        let inside_fence = matches!(fence.process_line(line), FenceStatus::Inside);

        if inside_fence {
            result.push_str(line);
            result.push('\n');
            continue;
        }

        let rewritten = rewrite_line_paths(line, source_dir, output_dir);
        result.push_str(&rewritten);
        result.push('\n');
    }
    result
}

/// Rewrite relative paths in a single Markdown line.
/// Handles inline images `![alt](path)`, inline links `[text](path)`,
/// and reference definitions `[id]: path`.
fn rewrite_line_paths(line: &str, source_dir: &Path, output_dir: &Path) -> String {
    // Scan for `](`pattern — matches both `[...](path)` and `![...](path)`.
    let mut result = String::with_capacity(line.len());
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b']' && i + 1 < bytes.len() && bytes[i + 1] == b'(' {
            // Found ]( — now backtrack to find the opening [ or ![
            let paren_start = i + 1;
            // Find the last [ before ]
            let bracket_start =
                line[..i].rfind('[').map(|p| if p > 0 && line.as_bytes()[p - 1] == b'!' { p - 1 } else { p });
            // Find closing )
            let rest = &line[paren_start + 1..];
            if let Some(paren_end_pos) = rest.find(')') {
                let target = &line[paren_start + 1..paren_start + 1 + paren_end_pos];
                if let Some(new_target) = rewrite_target(target, source_dir, output_dir) {
                    let prefix_end = if let Some(bs) = bracket_start {
                        // Keep from bracket_start to before paren_start
                        bs
                    } else {
                        i // fallback, keep the ]
                    };
                    result.push_str(&line[..prefix_end]);
                    // Reconstruct: [text](new) or ![alt](new)
                    let bracket_text = &line[prefix_end..i + 1]; // `[text]` or `![alt]`
                    result.push_str(bracket_text);
                    result.push('(');
                    result.push_str(&new_target);
                    result.push(')');
                    i = paren_start + 2 + paren_end_pos; // skip past the closing )
                } else {
                    // No rewrite needed but still copy what we have
                    result.push_str(&line[..=i]); // up to ]
                }
            } else {
                result.push(line.chars().nth(i).unwrap_or(']'));
            }
        } else {
            result.push(line.chars().nth(i).unwrap_or(' '));
        }
        i += 1;
    }

    // Also handle reference definitions: [id]: path
    let trimmed = result.trim_start();
    if trimmed.starts_with('[') && trimmed.contains("]: ") {
        if let Some(bracket_end) = trimmed.find(']') {
            let after = &trimmed[bracket_end + 1..];
            if let Some(after_colon) = after.strip_prefix(": ") {
                let target = after_colon.split_whitespace().next().unwrap_or("");
                if let Some(new_target) = rewrite_target(target, source_dir, output_dir) {
                    let indent = result.len() - trimmed.len();
                    let rest = after[2 + target.len()..].to_string();
                    result = format!("{}[{}]: {}{}", &result[..indent], &trimmed[1..bracket_end], new_target, rest);
                }
            }
        }
    }

    result
}

/// Determine whether a link target should be rewritten.
fn should_rewrite_target(target: &str) -> bool {
    if target.is_empty() {
        return false;
    }
    if target.starts_with('/') || target.starts_with("http://") || target.starts_with("https://") {
        return false;
    }
    if target.starts_with("mailto:") || target.starts_with("data:") || target.starts_with('#') {
        return false;
    }
    true
}

/// Compute the new relative path from `output_dir` to the same physical target
/// that `target` resolved to from `source_dir`.
fn rewrite_target(target: &str, source_dir: &Path, output_dir: &Path) -> Option<String> {
    if !should_rewrite_target(target) {
        return None;
    }
    // Compute target relative to source_dir
    let resolved = source_dir.join(target);
    // Compute relative path from output_dir to resolved
    let rel = relative_path_from(output_dir, &resolved)?;
    Some(rel)
}

/// Compute a relative path from `from` to `to`. Returns None if impossible.
fn relative_path_from(from: &Path, to: &Path) -> Option<String> {
    // Canonicalization is expensive and may fail; operate on normalized paths.
    use std::path::Component;
    let from_comps: Vec<Component<'_>> = from.components().collect();
    let to_comps: Vec<Component<'_>> = to.components().collect();

    // Find common prefix length
    let common_len = from_comps.iter().zip(to_comps.iter()).take_while(|(a, b)| a == b).count();

    let up_count = from_comps.len() - common_len;
    let mut result = String::new();
    for _ in 0..up_count {
        result.push_str("../");
    }
    for comp in &to_comps[common_len..] {
        if let Some(s) = comp.as_os_str().to_str() {
            if !result.is_empty() {
                result.push('/');
            }
            result.push_str(s);
        }
    }
    if result.is_empty() {
        result.push('.');
    }
    Some(result)
}

/// After a successful build, ensure the article is present in mind-index.yaml.
fn auto_index_article(project_path: &Path, article: &str, article_path: &Path) -> Result<()> {
    let mut index =
        crate::service::index::load(project_path).unwrap_or_else(|_| crate::model::index::IndexFile::create_default());
    let layout = config_svc::effective_layout(project_path)?;

    // Determine the relative article_path from the project root
    let rel_source = if article_path.is_dir() {
        let file_name = format!("{}.{}", article, defaults::MARKDOWN_EXTENSION);
        if article_path.join(&file_name).is_file() {
            article_path
                .join(file_name)
                .strip_prefix(project_path)
                .ok()
                .and_then(|p| p.to_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("{}/{}.{}", layout.articles, article, defaults::MARKDOWN_EXTENSION))
        } else {
            article_path
                .strip_prefix(project_path)
                .ok()
                .and_then(|p| p.to_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("{}/{}", layout.articles, article))
        }
    } else {
        // article_path is a file — strip project prefix to get the relative path
        article_path
            .strip_prefix(project_path)
            .ok()
            .and_then(|p| p.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("{}/{}.{}", layout.articles, article, defaults::MARKDOWN_EXTENSION))
    };

    // Check if article is already in the index
    let already_indexed =
        index.articles.as_ref().is_some_and(|articles| articles.iter().any(|a| a.article_path == rel_source));

    if !already_indexed {
        let project_name = util::dir_name(project_path);
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let articles = index.articles.get_or_insert_with(Vec::new);
        articles.push(crate::model::article::Article {
            title: article.replace('-', " "),
            project: project_name,
            article_type: crate::model::article::ArticleType::Blog,
            article_path: rel_source,
            status: crate::model::article::ArticleStatus::Published,
            created_at: now.clone(),
            updated_at: now,
            template_origin: None,
        });
        crate::service::index::save(project_path, &index)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::article::{Article, ArticleStatus, ArticleType};
    use crate::model::index::IndexFile;

    fn make_article(article_path: &str, title: &str) -> Article {
        Article {
            title: title.to_string(),
            project: "test".to_string(),
            article_type: ArticleType::Blog,
            article_path: article_path.to_string(),
            status: ArticleStatus::Draft,
            created_at: "2026-05-15T00:00:00Z".to_string(),
            updated_at: "2026-05-15T00:00:00Z".to_string(),
            template_origin: None,
        }
    }

    #[test]
    fn resolve_indexed_article_path_by_exact_key() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("docs/2026-05-monthly")).unwrap();
        std::fs::write(dir.path().join("docs/2026-05-monthly/01-team-okr.md"), "# okr\n").unwrap();
        let index = IndexFile {
            schema_version: "1".to_string(),
            articles: Some(vec![make_article("docs/2026-05-monthly", "2026 05 monthly")]),
            ..IndexFile::create_default()
        };
        index::save(dir.path(), &index).unwrap();

        let result = resolve_indexed_article_path(dir.path(), "2026-05-monthly").unwrap();
        assert!(result.exists());
        assert!(result.is_dir());
    }

    #[test]
    fn resolve_indexed_article_path_title_not_used_as_path_derivation() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("docs/team-updates")).unwrap();
        std::fs::write(dir.path().join("docs/team-updates/01-note.md"), "# note\n").unwrap();
        let index = IndexFile {
            schema_version: "1".to_string(),
            articles: Some(vec![make_article("docs/team-updates", "Team Updates")]),
            ..IndexFile::create_default()
        };
        index::save(dir.path(), &index).unwrap();

        // Lookup by key (not title) should work
        let result = resolve_indexed_article_path(dir.path(), "team-updates").unwrap();
        assert!(result.exists());

        // Lookup by display title should fail — title is not a search key
        let err = resolve_indexed_article_path(dir.path(), "Team Updates").unwrap_err();
        assert!(matches!(err, MfError::NotFound { .. }));
    }
}
