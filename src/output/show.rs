use std::path::PathBuf;

use unicode_width::UnicodeWidthStr;

pub struct ShowBlock {
    pub kind: &'static str,
    pub identity: String,
    pub fields: Vec<ShowField>,
    pub sections: Vec<ShowSection>,
}

pub struct ShowField {
    pub label: &'static str,
    pub value: ShowValue,
}

pub struct ShowSection {
    pub heading: &'static str,
    pub fields: Vec<ShowField>,
}

pub enum ShowValue {
    Text(String),
    Optional(Option<String>),
    Multiline(String),
    /// A repo-relative file path. When hyperlinks are enabled, rendered as a
    /// `file://` OSC 8 link resolved against `ShowOpts::repo_root`.
    Path(String),
}

#[derive(Default)]
pub struct ShowOpts {
    pub emit_hyperlinks: bool,
    pub repo_root: Option<PathBuf>,
}

impl ShowOpts {
    pub fn from_repo_root(repo_root: Option<&std::path::Path>) -> Self {
        let profile = super::capability::build_profile();
        let policy = crate::model::terminal::OutputRenderingPolicy::from_profile(
            &profile,
            crate::model::terminal::OutputFormat::Text,
        );
        Self { emit_hyperlinks: policy.emit_hyperlinks, repo_root: repo_root.map(|p| p.to_path_buf()) }
    }
}

pub fn render_text(block: &ShowBlock, opts: &ShowOpts) -> String {
    let max_label_w = block
        .fields
        .iter()
        .chain(block.sections.iter().flat_map(|section| section.fields.iter()))
        .map(|f| UnicodeWidthStr::width(format!("{}:", f.label).as_str()))
        .max()
        .unwrap_or(0);

    let mut out = String::new();
    for field in &block.fields {
        write_field(&mut out, field, max_label_w, 0, opts);
    }

    for section in &block.sections {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&format!("{}:", section.heading));
        out.push('\n');
        for field in &section.fields {
            write_field(&mut out, field, max_label_w, 2, opts);
        }
    }

    out
}

fn write_field(out: &mut String, field: &ShowField, max_label_w: usize, indent: usize, opts: &ShowOpts) {
    let label = format!("{}:", field.label);
    let label_w = UnicodeWidthStr::width(label.as_str());
    let padding = max_label_w.saturating_sub(label_w) + 2; // ":  " after key

    let prefix = " ".repeat(indent);
    out.push_str(&prefix);
    out.push_str(&label);
    for _ in 0..padding {
        out.push(' ');
    }

    match &field.value {
        ShowValue::Text(s) => out.push_str(s),
        ShowValue::Path(s) => {
            use super::link::render_path_link;
            let linked = render_path_link(s, opts.repo_root.as_deref(), opts.emit_hyperlinks);
            out.push_str(&linked);
        }
        ShowValue::Optional(Some(s)) => out.push_str(s),
        ShowValue::Optional(None) => out.push('-'),
        ShowValue::Multiline(s) => {
            out.push_str(s);
        }
    }
    out.push('\n');
}

pub fn json_envelope(block: &ShowBlock, extra_fields: serde_json::Map<String, serde_json::Value>) -> serde_json::Value {
    let mut map = extra_fields;
    map.insert("kind".to_string(), serde_json::Value::String(block.kind.to_string()));
    map.insert("identity".to_string(), serde_json::Value::String(block.identity.clone()));
    serde_json::Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_value_alignment() {
        let block = ShowBlock {
            kind: "project",
            identity: "demo".into(),
            fields: vec![
                ShowField { label: "Name", value: ShowValue::Text("demo".into()) },
                ShowField { label: "Path", value: ShowValue::Text("./projects/demo".into()) },
            ],
            sections: vec![],
        };
        let result = render_text(&block, &ShowOpts::default());
        let lines: Vec<&str> = result.lines().collect();
        let name_colon = lines[0].find(':').unwrap();
        let path_colon = lines[1].find(':').unwrap();
        let name_val = lines[0][name_colon..].find(|c: char| !c.is_whitespace()).unwrap();
        let path_val = lines[1][path_colon..].find(|c: char| !c.is_whitespace()).unwrap();
        let name_val_offset = name_colon + name_val;
        let path_val_offset = path_colon + path_val;
        assert_eq!(name_val_offset, path_val_offset);
    }

    #[test]
    fn optional_shows_dash() {
        let block = ShowBlock {
            kind: "source",
            identity: "test".into(),
            fields: vec![ShowField { label: "Description", value: ShowValue::Optional(None) }],
            sections: vec![],
        };
        let result = render_text(&block, &ShowOpts::default());
        assert!(result.contains('-'));
    }

    #[test]
    fn section_rendering() {
        let block = ShowBlock {
            kind: "project",
            identity: "demo".into(),
            fields: vec![],
            sections: vec![ShowSection {
                heading: "Layout",
                fields: vec![ShowField { label: "Articles", value: ShowValue::Text("docs".into()) }],
            }],
        };
        let result = render_text(&block, &ShowOpts::default());
        assert!(result.contains("Layout:"));
        assert!(result.contains("Articles"));
    }

    #[test]
    fn section_fields_participate_in_alignment() {
        let block = ShowBlock {
            kind: "project",
            identity: "demo".into(),
            fields: vec![ShowField { label: "Name", value: ShowValue::Text("demo".into()) }],
            sections: vec![ShowSection {
                heading: "Layout",
                fields: vec![ShowField { label: "Build output", value: ShowValue::Text("public".into()) }],
            }],
        };
        let result = render_text(&block, &ShowOpts::default());
        let lines: Vec<&str> = result.lines().collect();

        let top_value_offset = lines[0].find("demo").unwrap();
        let section_value_offset = lines[3].find("public").unwrap();
        assert_eq!(top_value_offset, section_value_offset - 2);
    }

    #[test]
    fn json_envelope_includes_kind_and_identity() {
        let block = ShowBlock { kind: "term", identity: "RAG".into(), fields: vec![], sections: vec![] };
        let mut extra = serde_json::Map::new();
        extra.insert("definition".to_string(), serde_json::Value::String("Retrieval...".into()));
        let result = json_envelope(&block, extra);
        assert_eq!(result["kind"], "term");
        assert_eq!(result["identity"], "RAG");
        assert_eq!(result["definition"], "Retrieval...");
    }

    #[test]
    fn path_value_renders_as_link() {
        let block = ShowBlock {
            kind: "article",
            identity: "docs/readme".into(),
            fields: vec![ShowField { label: "Path", value: ShowValue::Path("docs/readme.md".into()) }],
            sections: vec![],
        };
        let opts = ShowOpts { emit_hyperlinks: true, repo_root: Some(PathBuf::from("/repo")) };
        let result = render_text(&block, &opts);
        assert!(result.contains("\x1b]8;;file:///repo/docs/readme.md\x1b\\"));
    }

    #[test]
    fn path_value_no_hyperlinks_returns_plain() {
        let block = ShowBlock {
            kind: "article",
            identity: "docs/readme".into(),
            fields: vec![ShowField { label: "Path", value: ShowValue::Path("docs/readme.md".into()) }],
            sections: vec![],
        };
        let result = render_text(&block, &ShowOpts::default());
        assert!(!result.contains('\x1b'));
        assert!(result.contains("docs/readme.md"));
    }

    #[test]
    fn path_value_no_repo_root_returns_plain() {
        let block = ShowBlock {
            kind: "article",
            identity: "docs/readme".into(),
            fields: vec![ShowField { label: "Path", value: ShowValue::Path("docs/readme.md".into()) }],
            sections: vec![],
        };
        let opts = ShowOpts { emit_hyperlinks: true, repo_root: None };
        let result = render_text(&block, &opts);
        assert!(!result.contains('\x1b'));
        assert!(result.contains("docs/readme.md"));
    }
}
