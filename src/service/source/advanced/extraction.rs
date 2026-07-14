//! Deterministic text extraction from acquired content.
//!
//! Supports Markdown/text (passthrough), PDF (via pdf-extract),
//! HTML (via scraper), and RSS/Atom (via feed-rs).
//!
//! Each extraction produces canonical locators for the source format
//! so search results can point to exact positions within documents.

use crate::error::{MfError, Result};

use super::acquisition::AcquiredContent;

/// Extracted unit with format-specific locator.
#[derive(Debug, Clone)]
pub struct ExtractedUnit {
    /// Zero-based ordinal within the document.
    pub ordinal: u32,
    /// The extracted text content.
    pub text: String,
    /// JSON-serialized locator for this unit.
    pub locator_json: String,
    /// Sort key for deterministic ordering.
    pub locator_sort_key: String,
}

/// Extraction result.
pub struct ExtractionResult {
    /// The extractor identity string (e.g. "markdown-text-v1").
    pub extractor: String,
    /// Normalised text (all units joined).
    pub normalized_text: String,
    /// Ordered extraction units.
    pub units: Vec<ExtractedUnit>,
    /// Detected format label.
    pub format_label: String,
}

/// Extract text from acquired content based on its format.
///
/// Returns an error for unsupported formats. The caller should handle
/// per-registration failures gracefully (skip + diagnostic).
pub fn extract(content: &AcquiredContent) -> Result<ExtractionResult> {
    let format = detect_format(&content.raw_bytes, &content.acquisition_kind, &content.canonical_locator);

    match format {
        ContentFormat::Markdown | ContentFormat::Text => extract_text(content),
        ContentFormat::Pdf => extract_pdf(content),
        ContentFormat::Html => extract_html(content),
        ContentFormat::Rss => extract_rss(content),
        ContentFormat::Unknown => Err(MfError::advanced_store(
            format!("unsupported content format for: {}", content.canonical_locator),
            Some("only Markdown, text, PDF, HTML, and RSS/Atom feeds are supported".to_string()),
        )),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContentFormat {
    Markdown,
    Text,
    Pdf,
    Html,
    Rss,
    Unknown,
}

fn detect_format(raw: &[u8], acquisition_kind: &str, locator: &str) -> ContentFormat {
    // Check by extension
    let lower = locator.to_lowercase();
    if lower.ends_with(".pdf") {
        return ContentFormat::Pdf;
    }
    if lower.ends_with(".html") || lower.ends_with(".htm") {
        return ContentFormat::Html;
    }
    if lower.ends_with(".xml") || lower.ends_with(".rss") || lower.ends_with(".atom") {
        return ContentFormat::Rss;
    }
    if lower.ends_with(".md") || lower.ends_with(".markdown") || lower.ends_with(".mkd") {
        return ContentFormat::Markdown;
    }

    // Check by MIME magic bytes
    if raw.len() >= 4 && &raw[0..4] == b"%PDF" {
        return ContentFormat::Pdf;
    }
    if raw.len() >= 15 {
        let start = String::from_utf8_lossy(&raw[..15.min(raw.len())]).to_lowercase();
        if start.contains("<!doctype html") || start.contains("<html") {
            return ContentFormat::Html;
        }
        if start.contains("<?xml") && (start.contains("<rss") || start.contains("<feed") || start.contains("<atom")) {
            return ContentFormat::Rss;
        }
    }

    // Default: treat as plain text / Markdown
    if acquisition_kind == "http" { ContentFormat::Html } else { ContentFormat::Markdown }
}

fn extract_text(content: &AcquiredContent) -> Result<ExtractionResult> {
    let text = String::from_utf8_lossy(&content.raw_bytes).to_string();
    let lines: Vec<&str> = text.lines().collect();

    let mut units = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if line.trim().is_empty() {
            continue; // skip empty lines
        }
        let line_num = i as u64 + 1;
        units.push(ExtractedUnit {
            ordinal: units.len() as u32,
            text: line.to_string(),
            locator_json: serde_json::json!({"kind":"text","start_line":line_num,"end_line":line_num,"start_byte":0,"end_byte":line.len() as u64}).to_string(),
            locator_sort_key: format!("{:010}", line_num),
        });
    }

    Ok(ExtractionResult {
        extractor: "markdown-text-v1".to_string(),
        normalized_text: text.clone(),
        units,
        format_label: "markdown".to_string(),
    })
}

fn extract_pdf(content: &AcquiredContent) -> Result<ExtractionResult> {
    let text = pdf_extract::extract_text_from_mem(&content.raw_bytes)
        .map_err(|e| MfError::advanced_store(format!("PDF extraction failed: {e}"), None))?;

    let lines: Vec<&str> = text.lines().collect();
    let mut units = Vec::new();
    let page = 1u32;
    let mut char_offset = 0u64;

    for line in lines.iter() {
        if line.trim().is_empty() {
            continue;
        }
        let end_offset = char_offset + line.len() as u64;
        units.push(ExtractedUnit {
            ordinal: units.len() as u32,
            text: line.to_string(),
            locator_json: serde_json::json!({"kind":"pdf","page":page,"start_char":char_offset,"end_char":end_offset})
                .to_string(),
            locator_sort_key: format!("{:06}{:010}", page, char_offset),
        });
        char_offset = end_offset + 1; // +1 for newline
    }

    Ok(ExtractionResult {
        extractor: "pdf-extract-v1".to_string(),
        normalized_text: text,
        units,
        format_label: "pdf".to_string(),
    })
}

fn extract_html(content: &AcquiredContent) -> Result<ExtractionResult> {
    let html_str = String::from_utf8_lossy(&content.raw_bytes);
    let document = scraper::Html::parse_document(&html_str);

    // Extract text from body, preserving heading structure
    let body_selector = scraper::Selector::parse("body").unwrap();
    let mut units = Vec::new();
    let mut block_idx = 0u32;

    if let Some(body) = document.select(&body_selector).next() {
        for node in body.descendants() {
            if let Some(element) = node.value().as_element() {
                let tag = element.name().to_lowercase();
                // Collect block-level text
                if matches!(
                    tag.as_str(),
                    "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "li" | "td" | "th" | "blockquote" | "pre" | "code"
                ) {
                    let text: String = node
                        .descendants()
                        .filter_map(|n| n.value().as_text())
                        .map(|t| t.text.as_ref())
                        .collect::<Vec<_>>()
                        .join(" ")
                        .trim()
                        .to_string();
                    if !text.is_empty() && text.len() > 1 {
                        units.push(ExtractedUnit {
                            ordinal: units.len() as u32,
                            text,
                            locator_json:
                                serde_json::json!({"kind":"html","block":tag,"heading_path":[],"selector":null})
                                    .to_string(),
                            locator_sort_key: format!("{:010}", block_idx),
                        });
                        block_idx += 1;
                    }
                }
            }
        }
    }

    // Fallback: extract all text if no structured content found
    if units.is_empty() {
        let all_text: String = document
            .root_element()
            .descendants()
            .filter_map(|n| n.value().as_text())
            .map(|t| t.text.as_ref())
            .collect::<Vec<_>>()
            .join(" ");
        if !all_text.trim().is_empty() {
            units.push(ExtractedUnit {
                ordinal: 0,
                text: all_text.trim().to_string(),
                locator_json: r#"{"kind":"html","block":"body","heading_path":[],"selector":null}"#.to_string(),
                locator_sort_key: "0000000000".to_string(),
            });
        }
    }

    let normalized = units.iter().map(|u| u.text.as_str()).collect::<Vec<_>>().join("\n");

    Ok(ExtractionResult {
        extractor: "html-extract-v1".to_string(),
        normalized_text: normalized,
        units,
        format_label: "html".to_string(),
    })
}

fn extract_rss(content: &AcquiredContent) -> Result<ExtractionResult> {
    let feed = feed_rs::parser::parse(&content.raw_bytes[..])
        .map_err(|e| MfError::advanced_store(format!("RSS/Atom parsing failed: {e}"), None))?;

    let mut units = Vec::new();

    // Feed title + description as first unit
    let feed_title = match &feed.title {
        Some(t) => t.content.as_str(),
        None => "",
    };
    let feed_desc = match &feed.description {
        Some(d) => d.content.as_str(),
        None => "",
    };
    if !feed_title.is_empty() || !feed_desc.is_empty() {
        units.push(ExtractedUnit {
            ordinal: 0,
            text: format!("{feed_title}: {feed_desc}"),
            locator_json: r#"{"kind":"feed","entry_ordinal":0,"start_char":0,"end_char":0}"#.to_string(),
            locator_sort_key: "0000000000".to_string(),
        });
    }

    // Each entry as a unit
    for (i, entry) in feed.entries.iter().enumerate() {
        let title = match &entry.title {
            Some(t) => t.content.as_str(),
            None => "",
        };
        let summary = match &entry.summary {
            Some(s) => s.content.as_str(),
            None => "",
        };
        let content = match &entry.content {
            Some(c) => c.body.as_deref().unwrap_or(""),
            None => "",
        };
        let text = format!("{title}\n{summary}\n{content}").trim().to_string();
        if !text.is_empty() {
            units.push(ExtractedUnit {
                ordinal: units.len() as u32,
                text,
                locator_json: serde_json::json!({"kind":"feed","entry_id":entry.id,"entry_ordinal":i+1,"start_char":0,"end_char":0}).to_string(),
                locator_sort_key: format!("{:010}", i + 1),
            });
        }
    }

    let normalized = units.iter().map(|u| u.text.as_str()).collect::<Vec<_>>().join("\n");

    Ok(ExtractionResult {
        extractor: "feed-extract-v1".to_string(),
        normalized_text: normalized,
        units,
        format_label: "rss".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_markdown_text() {
        let content = AcquiredContent {
            raw_bytes: b"# Title\n\nParagraph one.\n\nParagraph two.\n".to_vec(),
            acquisition_kind: "local".to_string(),
            canonical_locator: "sources/test.md".to_string(),
            registered_location: "sources/test.md".to_string(),
        };
        let result = extract(&content).unwrap();
        assert_eq!(result.format_label, "markdown");
        assert!(result.units.len() >= 2);
        assert!(result.normalized_text.contains("Title"));
        assert!(result.normalized_text.contains("Paragraph one"));
    }

    #[test]
    fn extract_html_strips_tags() {
        let content = AcquiredContent {
            raw_bytes: b"<html><body><h1>Hello</h1><p>World</p></body></html>".to_vec(),
            acquisition_kind: "http".to_string(),
            canonical_locator: "https://example.com/page.html".to_string(),
            registered_location: "https://example.com/page.html".to_string(),
        };
        let result = extract(&content).unwrap();
        assert_eq!(result.format_label, "html");
        assert!(result.normalized_text.contains("Hello"));
        assert!(result.normalized_text.contains("World"));
    }

    #[test]
    fn detect_pdf_by_extension() {
        assert_eq!(detect_format(b"", "local", "paper.pdf"), ContentFormat::Pdf);
    }

    #[test]
    fn detect_pdf_by_magic() {
        assert_eq!(detect_format(b"%PDF-1.4\n...", "local", "unknown"), ContentFormat::Pdf);
    }

    #[test]
    fn extract_rss_feed() {
        let rss = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Test Feed</title>
    <description>A test feed</description>
    <item>
      <title>Entry 1</title>
      <description>First entry description</description>
    </item>
    <item>
      <title>Entry 2</title>
      <description>Second entry description</description>
    </item>
  </channel>
</rss>"#;
        let content = AcquiredContent {
            raw_bytes: rss.as_bytes().to_vec(),
            acquisition_kind: "http".to_string(),
            canonical_locator: "https://example.com/feed.xml".to_string(),
            registered_location: "https://example.com/feed.xml".to_string(),
        };
        let result = extract(&content).unwrap();
        assert_eq!(result.format_label, "rss");
        assert!(result.normalized_text.contains("Entry 1"));
        assert!(result.normalized_text.contains("Entry 2"));
    }
}
