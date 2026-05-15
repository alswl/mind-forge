//! Render service: template management, output resolution, and prompt assembly.
//!
//! This module implements the business logic for `mf render`. It does NOT
//! handle CLI dispatch, formatting, or I/O except through standard filesystem
//! operations needed to read built output and discover custom templates.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{MfError, Result};
use crate::model::config::MindConfig;
use crate::model::render::{
    GeneratedPrompt, HtmlForm, OutputContent, OutputInfo, RenderRequest, RenderScope, RenderTemplate, TemplateSource,
};
use crate::service::{config as config_svc, util as svc_util};

// ---------------------------------------------------------------------------
// Built-in templates (T007)
// ---------------------------------------------------------------------------

/// Return the two built-in render templates: `report` and `paper`.
pub fn built_in_templates() -> Vec<RenderTemplate> {
    vec![
        RenderTemplate {
            name: "report".to_string(),
            label: "Work report".to_string(),
            description: "Work/reporting presentation".to_string(),
            source: TemplateSource::BuiltIn,
            path: None,
            default: true,
        },
        RenderTemplate {
            name: "paper".to_string(),
            label: "Paper".to_string(),
            description: "Academic/paper-style presentation".to_string(),
            source: TemplateSource::BuiltIn,
            path: None,
            default: false,
        },
    ]
}

/// Return the built-in `report` template guidance body.
pub fn built_in_report_body() -> &'static str {
    "Render the supplied content as an HTML report page for work presentation.
Prioritize an executive summary, key conclusions, progress highlights, risks, and next steps.
Use scannable HTML sections with clear headings."
}

/// Return the built-in `paper` template guidance body.
pub fn built_in_paper_body() -> &'static str {
    "Render the supplied content as an academic/paper-style HTML page.
Include a title, abstract, sectioned argument with numbered sections, citation placeholders where appropriate, and a conclusion."
}

// ---------------------------------------------------------------------------
// Template validation helpers (T008)
// ---------------------------------------------------------------------------

/// Validate a template name is non-empty and contains no path separators.
pub fn validate_template_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(MfError::usage(
            "template name cannot be empty".to_string(),
            Some("use `mf render template list` to see available templates".to_string()),
        ));
    }
    if name.contains('/') || name.contains('\\') {
        return Err(MfError::usage(
            format!("invalid template name: '{name}'"),
            Some("template names must not contain path separators".to_string()),
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Context resolution helpers (T009)
// ---------------------------------------------------------------------------

/// Resolve the project path and load its config.
pub fn resolve_project_config(repo_root: &Path, project: Option<&str>, cwd: &Path) -> Result<(PathBuf, MindConfig)> {
    let project_path = svc_util::resolve_project(repo_root, project, cwd)?;
    let config = config_svc::load_project(&project_path, Some(repo_root))?.ok_or_else(|| {
        MfError::usage(
            format!("project missing mind.yaml in '{}'", project_path.display()),
            Some("run `mf config init` to create one".to_string()),
        )
    })?;
    Ok((project_path, config))
}

/// Resolve the built output path for an article, given project path and build config.
pub fn resolve_article_output(
    project_path: &Path,
    article: &str,
    output_dir: &str,
    format: &str,
) -> Result<OutputContent> {
    let output_path = project_path.join(output_dir).join(format!("{article}.{format}"));

    if !output_path.exists() {
        return Err(MfError::not_found(
            format!("output not found for article '{article}'"),
            Some(format!("run `mf build {article}` first")),
        ));
    }

    let metadata = fs::metadata(&output_path).map_err(MfError::Io)?;
    let size_bytes = metadata.len();

    if size_bytes == 0 {
        return Err(MfError::not_found(
            format!("output for article '{article}' is empty"),
            Some(format!("run `mf build {article}` to regenerate it")),
        ));
    }

    let content = fs::read_to_string(&output_path).map_err(MfError::Io)?;

    if content.trim().is_empty() {
        return Err(MfError::not_found(
            format!("output for article '{article}' contains only whitespace"),
            Some(format!("run `mf build {article}` to regenerate it")),
        ));
    }

    let rel_path = output_path.strip_prefix(project_path).unwrap_or(&output_path).to_string_lossy().to_string();

    Ok(OutputContent { path: rel_path, content, size_bytes })
}

/// Resolve the render template: look up by name in the available templates' keyed
/// by name -> (RenderTemplate, guidance_body).
pub fn resolve_template<'a>(
    template_name: &str,
    built_ins: &'a [(&'a RenderTemplate, &'a str)],
    custom_templates: &'a [(&'a RenderTemplate, &'a str)],
) -> Result<(&'a RenderTemplate, &'a str)> {
    validate_template_name(template_name)?;

    // Built-ins take precedence
    if let Some(found) = built_ins.iter().find(|(t, _)| t.name == template_name) {
        return Ok(*found);
    }
    if let Some(found) = custom_templates.iter().find(|(t, _)| t.name == template_name) {
        return Ok(*found);
    }

    let hint = "use `mf render template list` to see available templates".to_string();
    Err(MfError::usage(format!("unknown template: '{template_name}'"), Some(hint)))
}

/// Default template name.
pub fn default_template_name() -> &'static str {
    "report"
}

// ---------------------------------------------------------------------------
// Output discovery helpers
// ---------------------------------------------------------------------------

/// Discover built output files in a project's configured output directory.
pub fn discover_project_outputs(project_path: &Path, output_dir: &str, format: &str) -> Result<Vec<OutputContent>> {
    let out_path = project_path.join(output_dir);
    if !out_path.exists() || !out_path.is_dir() {
        return Err(MfError::not_found(
            format!("output directory '{}' does not exist", out_path.display()),
            Some("run `mf build` for the desired articles first".to_string()),
        ));
    }

    let mut outputs = Vec::new();

    for entry in fs::read_dir(&out_path).map_err(MfError::Io)? {
        let entry = entry.map_err(MfError::Io)?;
        let path = entry.path();
        if path.is_file() && path.extension().map(|e| e == format).unwrap_or(false) {
            let metadata = fs::metadata(&path).map_err(MfError::Io)?;
            let size_bytes = metadata.len();
            if size_bytes == 0 {
                continue;
            }
            let content = fs::read_to_string(&path).map_err(MfError::Io)?;
            if content.trim().is_empty() {
                continue;
            }
            let rel_path = path.strip_prefix(project_path).unwrap_or(&path).to_string_lossy().to_string();
            outputs.push(OutputContent { path: rel_path, content, size_bytes });
        }
    }

    // Sort deterministically
    outputs.sort_by(|a, b| a.path.cmp(&b.path));

    if outputs.is_empty() {
        return Err(MfError::not_found(
            format!("no renderable output found in '{}'", out_path.display()),
            Some("run `mf build` for the desired articles first".to_string()),
        ));
    }

    Ok(outputs)
}

// ---------------------------------------------------------------------------
// Prompt assembly
// ---------------------------------------------------------------------------

/// Assemble the full Agent-facing prompt from template guidance and output content.
pub fn assemble_prompt(template_body: &str, outputs: &[OutputContent], html_form: Option<HtmlForm>) -> String {
    let form_guidance = match html_form {
        Some(HtmlForm::Document) => "\n\nRender as a complete HTML document with <html>, <head>, and <body> tags.",
        Some(HtmlForm::Fragment) => {
            "\n\nRender as an HTML fragment that can be embedded in an existing page (no <html>, <head>, or <body> tags)."
        }
        None => "",
    };

    let mut prompt = String::new();
    prompt.push_str("You are an Agent rendering existing mf output into HTML.\n");
    prompt.push_str("\nTask:\n");
    prompt.push_str(template_body);
    prompt.push_str(form_guidance);
    prompt.push_str("\n\n");
    prompt.push_str(
        "mf is NOT rendering the final HTML. You, the Agent, are responsible for producing the HTML output.\n",
    );

    for output in outputs {
        prompt.push_str(&format!("\n--- BEGIN MF OUTPUT: {} ---\n", output.path));
        prompt.push_str(&output.content);
        if !output.content.ends_with('\n') {
            prompt.push('\n');
        }
        prompt.push_str(&format!("--- END MF OUTPUT: {} ---\n", output.path));
    }

    prompt
}

/// Generate the full `GeneratedPrompt` from request parameters.
#[allow(clippy::too_many_arguments)]
pub fn generate_prompt(
    request: &RenderRequest,
    project_path: &Path,
    output_dir: &str,
    format: &str,
    built_ins: &[(&RenderTemplate, &str)],
    custom_templates: &[(&RenderTemplate, &str)],
    html_form: Option<HtmlForm>,
) -> Result<GeneratedPrompt> {
    let (template, template_body) = resolve_template(&request.template, built_ins, custom_templates)?;

    let outputs = match request.scope {
        RenderScope::Article => {
            let article = request.article.as_deref().ok_or_else(|| {
                MfError::usage(
                    "article name is required for article-scope render",
                    Some("provide an article name: mf render <ARTICLE>".to_string()),
                )
            })?;
            vec![resolve_article_output(project_path, article, output_dir, format)?]
        }
        RenderScope::Project => discover_project_outputs(project_path, output_dir, format)?,
    };

    let prompt_text = assemble_prompt(template_body, &outputs, html_form);
    let output_infos: Vec<OutputInfo> =
        outputs.iter().map(|o| OutputInfo { path: o.path.clone(), size_bytes: o.size_bytes }).collect();

    Ok(GeneratedPrompt {
        prompt: prompt_text,
        template: template.name.clone(),
        template_source: template.source.clone(),
        scope: request.scope.clone(),
        project: request.project.clone(),
        article: request.article.clone(),
        outputs: output_infos,
    })
}

/// Return the default `.mind-forge/renders/` path relative to repo root.
pub fn custom_templates_dir(repo_root: &Path) -> PathBuf {
    repo_root.join(".mind-forge").join("renders")
}

/// Discover custom templates from `.mind-forge/renders/*.md`.
pub fn discover_custom_templates(repo_root: &Path) -> Result<Vec<(RenderTemplate, String)>> {
    let templates_dir = custom_templates_dir(repo_root);
    if !templates_dir.exists() {
        return Ok(Vec::new());
    }

    let mut templates = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(&templates_dir)
        .map_err(MfError::Io)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file() && e.path().extension().map(|ext| ext == "md").unwrap_or(false))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in &entries {
        let path = entry.path();
        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();

        // Validate name
        if name.is_empty() {
            continue; // skip files like `.md`
        }
        if name.contains('/') || name.contains('\\') {
            continue;
        }

        // Read content
        let content = fs::read_to_string(&path).map_err(MfError::Io)?;
        if content.trim().is_empty() {
            continue;
        }

        // Try to parse optional YAML frontmatter (between --- delimiters)
        let (label, description, body) = parse_template_frontmatter(&content, &name);

        let rel_path = path.strip_prefix(repo_root).unwrap_or(&path).to_string_lossy().to_string();

        // Check built-in name conflict
        let is_builtin = built_in_templates().iter().any(|t| t.name == name);
        if is_builtin {
            return Err(MfError::usage(
                format!(
                    "custom template '{name}' at '{rel_path}' conflicts with built-in template name; rename the file"
                ),
                Some("built-in template names are: report, paper".to_string()),
            ));
        }

        templates.push((
            RenderTemplate {
                name,
                label,
                description,
                source: TemplateSource::Custom,
                path: Some(rel_path),
                default: false,
            },
            body,
        ));
    }

    Ok(templates)
}

/// Parse optional YAML frontmatter from a Markdown file content.
/// Returns (label, description, body).
fn parse_template_frontmatter(content: &str, fallback_name: &str) -> (String, String, String) {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return (fallback_name.to_string(), String::new(), content.to_string());
    }

    // Find closing ---
    let after_first = &trimmed[3..];
    let end = match after_first.find("\n---") {
        Some(pos) => pos,
        None => return (fallback_name.to_string(), String::new(), content.to_string()),
    };

    let frontmatter_str = &trimmed[3..3 + end];
    let body = trimmed[3 + end + 4..].trim_start().to_string();

    let mut label = fallback_name.to_string();
    let mut description = String::new();

    for line in frontmatter_str.lines() {
        if let Some(val) = line.strip_prefix("label:") {
            label = val.trim().trim_matches('"').to_string();
        } else if let Some(val) = line.strip_prefix("description:") {
            description = val.trim().trim_matches('"').to_string();
        }
    }

    (label, description, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_template_name_empty() {
        assert!(validate_template_name("").is_err());
    }

    #[test]
    fn test_validate_template_name_with_slash() {
        assert!(validate_template_name("foo/bar").is_err());
    }

    #[test]
    fn test_validate_template_name_valid() {
        assert!(validate_template_name("report").is_ok());
    }

    #[test]
    fn test_built_in_templates_include_report_and_paper() {
        let templates = built_in_templates();
        assert!(templates.iter().any(|t| t.name == "report"));
        assert!(templates.iter().any(|t| t.name == "paper"));
        assert!(templates.iter().find(|t| t.name == "report").unwrap().default);
    }

    #[test]
    fn test_assemble_prompt_contains_output_boundaries() {
        let outputs =
            vec![OutputContent { path: "outputs/test.md".to_string(), content: "# Hello".to_string(), size_bytes: 8 }];
        let prompt = assemble_prompt("Render this content.", &outputs, None);
        assert!(prompt.contains("--- BEGIN MF OUTPUT: outputs/test.md ---"));
        assert!(prompt.contains("--- END MF OUTPUT: outputs/test.md ---"));
        assert!(prompt.contains("HTML"));
        assert!(prompt.contains("mf is NOT rendering"));
    }

    #[test]
    fn test_assemble_prompt_html_form_document() {
        let outputs =
            vec![OutputContent { path: "outputs/t.md".to_string(), content: "content".to_string(), size_bytes: 7 }];
        let prompt = assemble_prompt("test", &outputs, Some(HtmlForm::Document));
        assert!(prompt.contains("complete HTML document"));
    }

    #[test]
    fn test_assemble_prompt_html_form_fragment() {
        let outputs =
            vec![OutputContent { path: "outputs/t.md".to_string(), content: "content".to_string(), size_bytes: 7 }];
        let prompt = assemble_prompt("test", &outputs, Some(HtmlForm::Fragment));
        assert!(prompt.contains("HTML fragment"));
    }

    #[test]
    fn test_parse_frontmatter_with_label_and_description() {
        let content = "---\nlabel: Team Review\ndescription: Internal review page\n---\n\nTemplate body";
        let (label, desc, body) = parse_template_frontmatter(content, "team-review");
        assert_eq!(label, "Team Review");
        assert_eq!(desc, "Internal review page");
        assert_eq!(body.trim(), "Template body");
    }

    #[test]
    fn test_parse_frontmatter_without_frontmatter() {
        let (label, desc, body) = parse_template_frontmatter("Just content", "fallback");
        assert_eq!(label, "fallback");
        assert!(desc.is_empty());
        assert_eq!(body, "Just content");
    }
}
