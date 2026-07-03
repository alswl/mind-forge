use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::Utc;

use crate::defaults;
use crate::error::{MfError, Result};
use crate::model::article::{Article, ArticleStatus, ArticleType};
use crate::service::config as config_svc;
use crate::service::index;
use crate::service::util;

use super::template::{builtin_template, resolve_custom_template_path, split_template_into_blocks};
use super::{compute_typora_assets_path, effective_typora_enabled, inject_typora_front_matter};

/// Create a new article in the given project directory.
///
/// Handles both directory mode (default) and file mode (`--file`). The
/// template arg is resolved via [`builtin_template`] first and falls back
/// to a project-root-relative path lookup.
pub fn new_article(
    project_path: &Path,
    title: &str,
    template_arg: &str,
    file_mode: bool,
    tags: &[String],
    draft: bool,
    force: bool,
) -> Result<NewArticleResult> {
    let filename = util::to_filename(title);
    let layout = config_svc::effective_layout(project_path)?;
    let docs_dir = project_path.join(&layout.articles);
    fs::create_dir_all(&docs_dir).map_err(MfError::Io)?;

    // Resolve template
    let (resolved_body, article_type, template_label) = if let Some((body, at)) = builtin_template(template_arg) {
        (body.to_string(), at, template_arg.to_string())
    } else {
        let tmpl_path = resolve_custom_template_path(project_path, template_arg)?;
        match fs::read_to_string(&tmpl_path) {
            Ok(body) => (body, ArticleType::Blank, template_arg.to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(MfError::UnknownTemplate { name: template_arg.to_string() });
            }
            Err(e) => return Err(MfError::Io(e)),
        }
    };

    // Load full config for Typora plugin settings
    let config = config_svc::load_project(project_path, Some(project_path))?.unwrap_or_default();
    let plugins = config.plugins.as_ref();
    let typora_enabled = effective_typora_enabled(plugins);
    let typora_path: Option<String> = if typora_enabled {
        let file_dir = if file_mode {
            project_path.join(&layout.articles)
        } else {
            project_path.join(format!("{}/{}", layout.articles, filename))
        };
        Some(compute_typora_assets_path(project_path, &layout.assets, &file_dir))
    } else {
        None
    };

    let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let resolved =
        resolved_body.replace("{title}", title).replace("{created_at}", &now).replace("{tags}", &tags.join(", "));

    let files = if file_mode {
        write_article_file(
            project_path,
            &layout.articles,
            &filename,
            &resolved,
            typora_path.as_deref(),
            &now,
            title,
            article_type,
            draft,
            force,
        )
    } else {
        write_article_directory(
            project_path,
            &layout.articles,
            &filename,
            &resolved,
            typora_path.as_deref(),
            &now,
            title,
            article_type,
            draft,
            force,
        )
    }?;

    Ok(NewArticleResult {
        filename: filename.clone(),
        template: template_label,
        shape: if file_mode { "file".to_string() } else { "directory".to_string() },
        docs_dir: layout.articles,
        files,
        typora_front_matter_injected: typora_enabled,
        typora_copy_images_to: typora_path,
    })
}

/// Result of creating a new article, carrying metadata for the JSON envelope.
pub struct NewArticleResult {
    pub filename: String,
    pub template: String,
    pub shape: String,
    pub docs_dir: String,
    pub files: Vec<String>,
    pub typora_front_matter_injected: bool,
    pub typora_copy_images_to: Option<String>,
}

fn sibling_backup_path(target: &Path) -> PathBuf {
    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    let name = target.file_name().unwrap_or_default().to_string_lossy();
    let pid = std::process::id();
    let rand = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    parent.join(format!(".{name}.bak.{pid}.{rand}"))
}

#[allow(clippy::too_many_arguments)]
fn write_article_file(
    project_path: &Path,
    docs: &str,
    slug: &str,
    content: &str,
    typora_assets_path: Option<&str>,
    now: &str,
    title: &str,
    article_type: ArticleType,
    draft: bool,
    force: bool,
) -> Result<Vec<String>> {
    let file_path = project_path.join(format!("{docs}/{slug}.{}", defaults::MARKDOWN_EXTENSION));
    let dir_path = project_path.join(format!("{docs}/{slug}"));
    let file_name = format!("{slug}.{}", defaults::MARKDOWN_EXTENSION);

    // Cross-shape conflict check
    if dir_path.exists() {
        return Err(MfError::ShapeConflict {
            wanted_shape: "file".to_string(),
            existing_shape: "directory".to_string(),
            path: dir_path,
        });
    }

    let backup_path = if file_path.exists() {
        if force {
            let backup_path = sibling_backup_path(&file_path);
            fs::rename(&file_path, &backup_path).map_err(MfError::Io)?;
            Some(backup_path)
        } else {
            return Err(MfError::file_exists(file_path));
        }
    } else {
        None
    };

    let content = if let Some(assets_path) = typora_assets_path {
        inject_typora_front_matter(content, assets_path)
    } else {
        content.to_string()
    };

    if let Err(e) = fs::write(&file_path, content).map_err(MfError::Io) {
        if let Some(backup_path) = &backup_path {
            let _ = fs::rename(backup_path, &file_path);
        }
        return Err(e);
    }

    let article_path = format!("{docs}/{file_name}");
    match write_index_entry(project_path, title, article_type, &article_path, now, draft, force) {
        Ok(()) => {
            if let Some(backup_path) = backup_path {
                let _ = fs::remove_file(backup_path);
            }
            Ok(vec![file_name])
        }
        Err(e) => {
            let _ = fs::remove_file(&file_path);
            if let Some(backup_path) = &backup_path {
                let _ = fs::rename(backup_path, &file_path);
            }
            Err(e)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn write_article_directory(
    project_path: &Path,
    docs: &str,
    slug: &str,
    content: &str,
    typora_assets_path: Option<&str>,
    now: &str,
    title: &str,
    article_type: ArticleType,
    draft: bool,
    force: bool,
) -> Result<Vec<String>> {
    let dir_path = project_path.join(format!("{docs}/{slug}"));
    let file_path = project_path.join(format!("{docs}/{slug}.{}", defaults::MARKDOWN_EXTENSION));

    // Cross-shape conflict check
    if file_path.exists() {
        return Err(MfError::ShapeConflict {
            wanted_shape: "directory".to_string(),
            existing_shape: "file".to_string(),
            path: file_path,
        });
    }

    let backup_path = if dir_path.exists() {
        if force {
            let backup_path = sibling_backup_path(&dir_path);
            fs::rename(&dir_path, &backup_path).map_err(MfError::Io)?;
            Some(backup_path)
        } else {
            return Err(MfError::file_exists(dir_path));
        }
    } else {
        None
    };

    let mut blocks = split_template_into_blocks(content)?;
    if let Some(assets_path) = typora_assets_path {
        for (_, body) in &mut blocks {
            *body = inject_typora_front_matter(body, assets_path);
        }
    }
    let files: Vec<String> = blocks.iter().map(|(filename, _)| filename.clone()).collect();
    let block_refs: Vec<(&str, &str)> = blocks.iter().map(|(f, b)| (f.as_str(), b.as_str())).collect();
    if let Err(e) = util::atomic_write_directory(&dir_path, &block_refs) {
        if let Some(backup_path) = &backup_path {
            let _ = fs::rename(backup_path, &dir_path);
        }
        return Err(e);
    }

    let article_path = format!("{docs}/{slug}");
    match write_index_entry(project_path, title, article_type, &article_path, now, draft, force) {
        Ok(()) => {
            if let Some(backup_path) = backup_path {
                let _ = fs::remove_dir_all(backup_path);
            }
            Ok(files)
        }
        Err(e) => {
            let _ = fs::remove_dir_all(&dir_path);
            if let Some(backup_path) = &backup_path {
                let _ = fs::rename(backup_path, &dir_path);
            }
            Err(e)
        }
    }
}

fn write_index_entry(
    project_path: &Path,
    title: &str,
    article_type: ArticleType,
    article_path: &str,
    now: &str,
    draft: bool,
    force: bool,
) -> Result<()> {
    let project_name = util::dir_name(project_path);
    let mut index = index::load(project_path)?;
    let articles = index.articles.get_or_insert_with(Vec::new);
    let status = if draft { ArticleStatus::Draft } else { ArticleStatus::Published };

    if force {
        articles.retain(|a| a.article_path != article_path);
    }

    articles.push(Article {
        title: title.to_string(),
        project: project_name,
        article_type,
        article_path: article_path.to_string(),
        status,
        created_at: now.to_string(),
        updated_at: now.to_string(),
        template_origin: None,
    });
    index::save(project_path, &index)?;
    Ok(())
}
