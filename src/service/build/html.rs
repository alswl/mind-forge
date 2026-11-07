//! HTML is rendered only from the assembled, public Markdown. Templates are
//! static body fragments; the renderer owns the document head and runtime.
use crate::error::{MfError, Result};
use base64::Engine;
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use sha2::{Digest, Sha256};
use std::path::Path;

const RUNTIME: &str = include_str!("html.js");
const STYLE: &str = include_str!("html.css");

fn clean(value: &str) -> String {
    ammonia::Builder::default()
        .add_tags(["figure", "figcaption", "details", "summary"])
        .add_generic_attributes(["class", "aria-live"])
        .add_tag_attributes("details", ["open"])
        .clean(value)
        .to_string()
}

pub fn template(project: &Path, configured: Option<&str>) -> Result<String> {
    let value = if let Some(path) = configured {
        let path = crate::service::mind::checked_path(project, path)?;
        std::fs::read_to_string(path)?
    } else {
        "<main class=\"mf-content\">{{content}}</main>".into()
    };
    let value = clean(&value);
    let parsed = scraper::Html::parse_fragment(&value);
    let standalone = parsed
        .tree
        .nodes()
        .filter(|node| node.value().as_text().is_some_and(|text| text.trim() == "{{content}}"))
        .count();
    if value.matches("{{content}}").count() != 1 || standalone != 1 {
        return Err(MfError::Lifecycle {
            code: "invalid_html_template",
            message: "HTML body template must contain exactly one standalone {{content}} text placeholder after safety filtering"
                .into(),
        });
    }
    Ok(value)
}

fn escape(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}

pub fn render(markdown: &str, title: &str, template: &str) -> String {
    let markdown = crate::service::util::markdown::strip_front_matter(markdown);
    let parser = Parser::new_ext(&markdown, Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH);
    let mut events = Vec::new();
    let mut diagram: Option<String> = None;
    for event in parser {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(ref language))) if language.trim() == "mermaid" => {
                diagram = Some(String::new());
            }
            Event::End(TagEnd::CodeBlock) if diagram.is_some() => {
                let source = escape(&diagram.take().unwrap());
                events.push(Event::Html(format!("<figure class=\"mf-diagram\"><p class=\"mf-diagram-status\" aria-live=\"polite\">Mermaid diagram is not rendered yet. JavaScript and CDN access are required; source follows.</p><div class=\"mf-diagram-output\"></div><details open><summary>Diagram source</summary><pre><code>{source}</code></pre></details></figure>").into()));
            }
            Event::Text(text) if diagram.is_some() => diagram.as_mut().unwrap().push_str(&text),
            // Raw document HTML is visible text, never active markup.
            Event::Html(text) | Event::InlineHtml(text) => events.push(Event::Text(text)),
            other => events.push(other),
        }
    }
    let mut body = String::new();
    pulldown_cmark::html::push_html(&mut body, events.into_iter());
    let escaped_title = escape(title);
    let mut parts = template.split("{{content}}");
    let before = parts.next().unwrap_or_default().replace("{{title}}", &escaped_title);
    let after = parts.next().unwrap_or_default().replace("{{title}}", &escaped_title);
    let body = clean(&format!("{before}{body}{after}"));
    let hash = base64::engine::general_purpose::STANDARD.encode(Sha256::digest(RUNTIME.as_bytes()));
    format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src 'sha256-{hash}' https://cdn.jsdelivr.net/npm/mermaid@11.12.0/; style-src 'unsafe-inline'; img-src 'self' https: http:; font-src 'self'; connect-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'\"><title>{escaped_title}</title><style>{STYLE}</style></head><body>{body}<script type=\"module\">{RUNTIME}</script></body></html>\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn template_title_substitution_cannot_introduce_executable_urls() {
        let html = render("# Public", "javascript:alert(1)", "<a href=\"{{title}}\">Link</a><main>{{content}}</main>");
        assert!(!html.contains("href=\"javascript:"));
        assert!(html.contains("<h1>Public</h1>"));
    }
}
