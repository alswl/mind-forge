pub mod article;
pub mod asset;
pub mod config;
pub mod index;
pub mod lifecycle;
pub mod project;
pub mod prompt;
pub mod publish;
pub mod source;
pub mod term;
pub mod terminal;
pub mod thinking;

pub mod manifest;
pub mod publisher;
pub mod render;

/// Trait for resources that have a stable identity for list/show round-trips.
#[allow(dead_code)]
pub trait Resource {
    const KIND: &'static str;
    fn identity(&self) -> String;
}

impl Resource for manifest::ProjectEntry {
    const KIND: &'static str = "project";
    fn identity(&self) -> String {
        self.name.clone()
    }
}

impl Resource for project::ProjectListEntry {
    const KIND: &'static str = "project";
    fn identity(&self) -> String {
        self.name.clone()
    }
}

impl Resource for article::Article {
    const KIND: &'static str = "article";
    fn identity(&self) -> String {
        self.article_path.clone()
    }
}

impl Resource for source::Source {
    const KIND: &'static str = "source";
    fn identity(&self) -> String {
        self.name.clone()
    }
}

impl Resource for asset::Asset {
    const KIND: &'static str = "asset";
    fn identity(&self) -> String {
        self.path.clone()
    }
}

impl Resource for term::Term {
    const KIND: &'static str = "term";
    fn identity(&self) -> String {
        self.term.clone()
    }
}

impl Resource for publisher::PublisherView {
    const KIND: &'static str = "publish_target";
    fn identity(&self) -> String {
        self.name.clone()
    }
}

impl Resource for render::RenderTemplate {
    const KIND: &'static str = "render_template";
    fn identity(&self) -> String {
        self.name.clone()
    }
}

impl Resource for term::Correction {
    const KIND: &'static str = "term_correction";
    fn identity(&self) -> String {
        format!("{}::{}", self.original, self.correct)
    }
}

impl Resource for prompt::Prompt {
    const KIND: &'static str = "prompt";
    fn identity(&self) -> String {
        self.path.clone()
    }
}

impl Resource for thinking::Thinking {
    const KIND: &'static str = "thinking";
    fn identity(&self) -> String {
        self.path.clone()
    }
}

#[cfg(test)]
mod resource_tests {
    use super::*;

    #[test]
    fn project_entry_identity() {
        let entry = manifest::ProjectEntry {
            name: "demo".into(),
            path: "projects/demo".into(),
            created_at: String::new(),
            archived_at: None,
        };
        assert_eq!(entry.identity(), "demo");
        assert_eq!(manifest::ProjectEntry::KIND, "project");
    }

    #[test]
    fn article_identity() {
        let article = article::Article {
            title: "Test".into(),
            project: "p".into(),
            article_type: article::ArticleType::Blank,
            article_path: "docs/test.md".into(),
            status: article::ArticleStatus::Draft,
            created_at: String::new(),
            updated_at: String::new(),
            template_origin: None,
        };
        assert_eq!(article.identity(), "docs/test.md");
    }

    #[test]
    fn source_identity() {
        let source = source::Source {
            name: "report".into(),
            kind: source::FileKind::File,
            source_kind: None,
            url: None,
            path: Some("sources/report.pdf".into()),
            tags: vec![],
            added_at: String::new(),
            updated_at: String::new(),
        };
        assert_eq!(source.identity(), "report");
    }

    #[test]
    fn asset_identity() {
        let asset = asset::Asset {
            name: "diagram".into(),
            kind: asset::AssetKind::Image,
            path: "assets/diagram.png".into(),
            size: 1024,
            hash: "abc".into(),
            tags: vec![],
            added_at: String::new(),
        };
        assert_eq!(asset.identity(), "assets/diagram.png");
    }

    #[test]
    fn term_identity() {
        let term = term::Term {
            term: "RAG".into(),
            definition: Some("def".into()),
            description: None,
            confidence: None,
            aliases: vec![],
            tags: vec![],
            corrections: vec![],
        };
        assert_eq!(term.identity(), "RAG");
    }

    #[test]
    fn correction_identity() {
        let c = term::Correction {
            original: "Rag".into(),
            correct: "RAG".into(),
            r#match: term::MatchKind::Word,
            fix: term::FixKind::Required,
            boundary: term::Boundary::Loose,
            pinyin: None,
        };
        assert_eq!(c.identity(), "Rag::RAG");
    }
}
