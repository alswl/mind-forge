//! Backend-aware repository catalog discovery.
//!
//! In legacy mode, Source registrations are read from project
//! `mind-index.yaml.sources`. In Lance mode, the pinned LanceDB
//! `registrations` table is the primary authority, intersected with
//! the root active-project catalog from `minds.yaml`.

use std::collections::HashMap;
use std::path::Path;

use arrow_array::{Array, StringArray};

use crate::error::Result;
use crate::model::manifest::SourceBackend;
use crate::model::source_advanced::{ContentKind, DocumentContext, Relation, RelationType};

use super::config::ResolvedSourceConfig;
use super::lance_store::LanceStore;

// ── Catalog ────────────────────────────────────────────────────────────────

/// A resolved catalog of all live Source registrations across active projects.
#[derive(Debug, Clone)]
pub struct SourceCatalog {
    pub backend: SourceBackend,
    /// Number of active projects in the repository.
    pub active_projects: usize,
    /// Number of live registrations in scope.
    pub registration_count: usize,
    /// Whether the catalog was read from Lance primary (true) or legacy YAML (false).
    pub from_lance_primary: bool,
}

/// A lightweight view of a Source registration for listing/indexing.
#[derive(Debug, Clone)]
pub struct CatalogRegistration {
    pub registration_key: String,
    pub project_key: String,
    pub project_identity: String,
    pub project_path: String,
    pub source_identity: String,
    pub source_type: String,
    pub source_kind: Option<String>,
    pub registered_location: String,
    pub tags_json: String,
    pub labels_json: String,
    pub annotations_json: String,
    pub state: String,
    /// Serialized `DocumentContext` (schema v2); `None` before enrichment.
    pub context_json: Option<String>,
    /// Serialized `ImportProvenance` for source bindings (schema v2).
    pub imported_by_json: Option<String>,
}

impl SourceCatalog {
    /// Discover the active Source catalog based on the resolved backend config.
    ///
    /// In legacy mode, this is a no-op placeholder — the caller should use
    /// existing project-level Source indexing. In Lance mode, it reads from
    /// the pinned LanceDB store.
    pub fn discover(config: &ResolvedSourceConfig, _repo_root: &Path) -> Result<Self> {
        match config.backend {
            SourceBackend::Legacy => Ok(Self {
                backend: SourceBackend::Legacy,
                active_projects: 0,
                registration_count: 0,
                from_lance_primary: false,
            }),
            SourceBackend::Lance => {
                // In Lance mode, registrations are read from the primary table.
                // The actual discovery is deferred to the caller — this struct
                // serves as a context marker.
                Ok(Self {
                    backend: SourceBackend::Lance,
                    active_projects: 0,
                    registration_count: 0,
                    from_lance_primary: true,
                })
            }
        }
    }

    /// Return the list of catalog registrations.
    ///
    /// In Lance mode this queries the pinned snapshot. In legacy mode it
    /// returns an empty list (the caller uses project-level indexing).
    pub fn registrations(&self, store: Option<&LanceStore>) -> Result<Vec<CatalogRegistration>> {
        if !self.from_lance_primary {
            return Ok(Vec::new());
        }
        let store = store.ok_or_else(|| {
            crate::error::MfError::advanced_store(
                "Lance primary catalog requested without an open store".to_string(),
                None,
            )
        })?;
        let mut registrations = Vec::new();
        for batch in store.scan_rows("registrations")? {
            let column = |name| -> Result<&StringArray> {
                batch.column_by_name(name).and_then(|column| column.as_any().downcast_ref::<StringArray>()).ok_or_else(
                    || crate::error::MfError::advanced_store(format!("registrations table missing '{name}'"), None),
                )
            };
            let keys = column("registration_key")?;
            let project_keys = column("project_key")?;
            let projects = column("project_identity")?;
            let paths = column("project_path")?;
            let sources = column("source_identity")?;
            let types = column("source_type")?;
            let source_kinds = column("source_kind")?;
            let locations = column("registered_location")?;
            let tags = column("tags_json")?;
            let labels = column("labels_json")?;
            let annotations = column("annotations_json")?;
            let states = column("state")?;
            // Schema v2 columns are read optionally: `rebuild` reads a v1 table
            // (which lacks them) before regenerating with the current schema.
            let optional = |name: &str| -> Option<&StringArray> {
                batch.column_by_name(name).and_then(|c| c.as_any().downcast_ref::<StringArray>())
            };
            let contexts = optional("context_json");
            let imported = optional("imported_by_json");
            let read_opt = |array: Option<&StringArray>, row: usize| -> Option<String> {
                array.and_then(|a| (!a.is_null(row)).then(|| a.value(row).to_string()))
            };
            for row in 0..batch.num_rows() {
                registrations.push(CatalogRegistration {
                    registration_key: keys.value(row).to_string(),
                    project_key: project_keys.value(row).to_string(),
                    project_identity: projects.value(row).to_string(),
                    project_path: paths.value(row).to_string(),
                    source_identity: sources.value(row).to_string(),
                    source_type: types.value(row).to_string(),
                    source_kind: (!source_kinds.is_null(row)).then(|| source_kinds.value(row).to_string()),
                    registered_location: locations.value(row).to_string(),
                    tags_json: tags.value(row).to_string(),
                    labels_json: labels.value(row).to_string(),
                    annotations_json: annotations.value(row).to_string(),
                    state: states.value(row).to_string(),
                    context_json: read_opt(contexts, row),
                    imported_by_json: read_opt(imported, row),
                });
            }
        }
        registrations.sort_by(|a, b| {
            a.project_path.cmp(&b.project_path).then_with(|| a.source_identity.cmp(&b.source_identity))
        });
        Ok(registrations)
    }
}

/// Classify a repository-relative path against discovery exclusion rules
/// (spec 071, FR-002). Returns `Some(reason)` when the path is plumbing, a
/// computed cache, a build artifact, or an asset that must never be indexed as
/// content; `None` when the path is eligible for indexing. The `excluded`
/// reason is surfaced in the sync report so exclusions are auditable, never
/// silent (FR-003).
pub fn discovery_exclusion(rel_path: &str) -> Option<&'static str> {
    let p = rel_path.replace('\\', "/");
    // Computed cache + repository plumbing (rebuildable, not authored content).
    if p == ".mind"
        || p.starts_with(".mind/")
        || p.contains("/.mind/")
        || p == ".mind-forge"
        || p.starts_with(".mind-forge/")
        || p.contains("/.mind-forge/")
        || p.ends_with("mind-index.yaml")
        || p.ends_with("mind.yaml")
        || p == "minds.yaml"
    {
        return Some("excluded");
    }
    // Build artifacts.
    if p.contains("/dist/") || p.starts_with("dist/") || p.contains("/build/") || p.starts_with("build/") {
        return Some("excluded");
    }
    // Binary / asset extensions (not text content).
    const BINARY_EXTS: &[&str] = &[
        "png", "jpg", "jpeg", "gif", "webp", "svg", "pdf", "zip", "gz", "tar", "mp4", "mov", "mp3", "wav", "bin",
        "woff", "woff2", "ttf", "otf", "ico",
    ];
    if let Some(ext) = std::path::Path::new(&p).extension().and_then(|e| e.to_str())
        && BINARY_EXTS.contains(&ext.to_ascii_lowercase().as_str())
    {
        return Some("binary");
    }
    None
}

/// Discover repository-authored article artifacts as RAG registrations.
/// Article rows are derived from local files and are intentionally not
/// exported to the legacy Source projection or source-only bundles.
pub fn discover_article_registrations(repo_root: &Path) -> Vec<CatalogRegistration> {
    let mut out = Vec::new();
    let projects = repo_root.join("projects");
    let Ok(entries) = std::fs::read_dir(projects) else { return out };
    for entry in entries.flatten() {
        let project_path = entry.path();
        if !project_path.is_dir() {
            continue;
        }
        let project_identity = entry.file_name().to_string_lossy().to_string();
        let project_rel =
            project_path.strip_prefix(repo_root).unwrap_or(&project_path).to_string_lossy().replace('\\', "/");
        let project_key = super::identity::project_key(&project_rel);
        for (directory, kind, prefix) in
            [("prompts", "article_prompt", "prompt:"), ("thinking", "article_thinking", "thinking:")]
        {
            collect_article_files(
                &project_path,
                &project_key,
                &project_identity,
                &project_rel,
                &project_path.join(directory),
                kind,
                prefix,
                &mut out,
            );
        }
        // Articles are the authored source under the project's configured
        // articles dir (`docs/` by default), NOT the `outputs/` build artifacts
        // — an article is written before, and often without, being built. Use
        // the canonical article enumeration (respects layout config, excludes
        // asset dirs, handles single-file and block articles).
        if let Ok(articles) = crate::service::article::list_articles(&project_path) {
            for article in articles {
                let location = article.article_path;
                if location.is_empty() {
                    continue;
                }
                let name = location.rsplit('/').next().unwrap_or(&location).trim_end_matches(".md");
                out.push(CatalogRegistration {
                    registration_key: super::identity::registration_key(&project_key, "article", &location),
                    project_key: project_key.clone(),
                    project_identity: project_identity.clone(),
                    project_path: project_rel.clone(),
                    source_identity: format!("article:{name}"),
                    source_type: "article".into(),
                    source_kind: Some("article".into()),
                    registered_location: location,
                    tags_json: "[]".into(),
                    labels_json: "{}".into(),
                    annotations_json: "{}".into(),
                    state: "live".into(),
                    context_json: None,
                    imported_by_json: None,
                });
            }
        }
    }
    out.sort_by(|a, b| a.registration_key.cmp(&b.registration_key));
    out
}

/// Discover one `project` registration per active project (spec 071 US2). The
/// searchable content is the project's `mind.yaml` goal/description; the row is
/// single-owner so its context participates in matching.
pub fn discover_project_registrations(repo_root: &Path) -> Vec<CatalogRegistration> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(repo_root.join("projects")) else { return out };
    for entry in entries.flatten() {
        let project_path = entry.path();
        if !project_path.is_dir() || !project_path.join("mind.yaml").is_file() {
            continue;
        }
        let project_identity = entry.file_name().to_string_lossy().to_string();
        let project_rel =
            project_path.strip_prefix(repo_root).unwrap_or(&project_path).to_string_lossy().replace('\\', "/");
        let project_key = super::identity::project_key(&project_rel);
        out.push(CatalogRegistration {
            registration_key: super::identity::registration_key(&project_key, "project", "mind.yaml"),
            project_key,
            project_identity: project_identity.clone(),
            project_path: project_rel,
            source_identity: format!("project:{project_identity}"),
            source_type: "project".into(),
            source_kind: Some("project".into()),
            registered_location: "mind.yaml".into(),
            tags_json: "[]".into(),
            labels_json: "{}".into(),
            annotations_json: "{}".into(),
            state: "live".into(),
            context_json: None,
            imported_by_json: None,
        });
    }
    out.sort_by(|a, b| a.registration_key.cmp(&b.registration_key));
    out
}

/// Discover one `term` registration per repository-global term (spec 071 US2).
/// Terms are repo-scoped, so they carry a synthetic `(repository)` project. The
/// searchable content (definition + aliases + description) is assembled at sync.
pub fn discover_term_registrations(repo_root: &Path) -> Vec<CatalogRegistration> {
    let Ok(terms) = crate::service::term::global::load_terms(repo_root) else { return Vec::new() };
    let project_key = super::identity::project_key(".");
    let mut out = Vec::new();
    for term in terms {
        out.push(CatalogRegistration {
            registration_key: super::identity::registration_key(&project_key, "term", &term.term),
            project_key: project_key.clone(),
            project_identity: "(repository)".into(),
            project_path: ".".into(),
            source_identity: format!("term:{}", term.term),
            source_type: "term".into(),
            source_kind: Some("term".into()),
            registered_location: term.term.clone(),
            tags_json: "[]".into(),
            labels_json: "{}".into(),
            annotations_json: "{}".into(),
            state: "live".into(),
            context_json: None,
            imported_by_json: None,
        });
    }
    out.sort_by(|a, b| a.registration_key.cmp(&b.registration_key));
    out
}

#[allow(clippy::too_many_arguments)]
fn collect_article_files(
    project_path: &Path,
    project_key: &str,
    project_identity: &str,
    project_rel: &str,
    directory: &Path,
    kind: &str,
    prefix: &str,
    out: &mut Vec<CatalogRegistration>,
) {
    let Ok(files) = std::fs::read_dir(directory) else { return };
    for file in files.flatten() {
        let path = file.path();
        if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let Ok(location) = path.strip_prefix(project_path).map(|p| p.to_string_lossy().replace('\\', "/")) else {
            continue;
        };
        let name = path.file_stem().map(|s| s.to_string_lossy()).unwrap_or_default();
        out.push(CatalogRegistration {
            registration_key: super::identity::registration_key(project_key, kind, &location),
            project_key: project_key.to_string(),
            project_identity: project_identity.to_string(),
            project_path: project_rel.to_string(),
            source_identity: format!("{prefix}{name}"),
            source_type: kind.to_string(),
            source_kind: Some(kind.to_string()),
            registered_location: location,
            tags_json: "[]".into(),
            labels_json: "{}".into(),
            annotations_json: "{}".into(),
            state: "live".into(),
            context_json: None,
            imported_by_json: None,
        });
    }
}

/// Repository identity (outermost attribution) for `DocumentContext`. The minds
/// manifest carries no explicit name, so the repository directory name is used.
/// Shared by discovery enrichment and retrieval so persisted and synthesized
/// contexts agree.
pub fn repository_identity(repo_root: &Path) -> String {
    repo_root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "minds".to_string())
}

/// Enrich single-owner registrations (article/article_prompt/article_thinking)
/// with a persisted `DocumentContext`: project attribution + goal, article
/// lifecycle status, internal-link relations, and prompt/thinking siblings
/// (spec 071 FR-005/006/009). Source rows are left untouched — their context is
/// provenance-only and synthesized at retrieval.
pub fn enrich_single_owner_contexts(repo_root: &Path, registrations: &mut [CatalogRegistration]) {
    let repository = repository_identity(repo_root);
    let mut goal_cache: HashMap<String, Option<String>> = HashMap::new();
    for reg in registrations.iter_mut() {
        let content_kind = ContentKind::from_registration_kind(&reg.source_type);
        if !content_kind.is_single_owner() {
            continue;
        }
        let project_dir = repo_root.join(&reg.project_path);
        let project_goal =
            goal_cache.entry(reg.project_path.clone()).or_insert_with(|| read_project_goal(&project_dir)).clone();
        let article_path = project_dir.join(&reg.registered_location);
        let body = std::fs::read_to_string(&article_path).unwrap_or_default();
        let lifecycle_status = (reg.source_type == "article").then(|| front_matter_field(&body, "status")).flatten();
        let mut relations = Vec::new();
        if reg.source_type == "article" {
            // Internal links resolve relative to the article's own directory.
            let article_dir = article_path.parent().unwrap_or(&project_dir);
            collect_link_relations(&body, article_dir, &mut relations);
            collect_sibling_relations(&project_dir, &reg.source_identity, &mut relations);
        }
        let mut context = DocumentContext {
            repository: repository.clone(),
            project_identity: reg.project_identity.clone(),
            project_goal,
            content_kind,
            lifecycle_status,
            relations,
            imported_by: None,
            single_owner: true,
        };
        context.normalize();
        reg.context_json = serde_json::to_string(&context).ok();
    }
}

/// Read an optional free-form `goal` (or `description`) string from a project's
/// `mind.yaml`. Missing file or field yields `None` (Constitution VI tolerance).
fn read_project_goal(project_dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(project_dir.join("mind.yaml")).ok()?;
    let value: serde_yaml::Value = serde_yaml::from_str(&text).ok()?;
    let map = value.as_mapping()?;
    for key in ["goal", "description", "objective"] {
        if let Some(s) = map.get(serde_yaml::Value::String(key.into())).and_then(|v| v.as_str())
            && !s.trim().is_empty()
        {
            return Some(s.trim().to_string());
        }
    }
    None
}

/// Parse a single scalar field from a leading YAML front-matter block
/// (`---\n…\n---`). Returns `None` when absent.
fn front_matter_field(body: &str, field: &str) -> Option<String> {
    let rest = body.strip_prefix("---")?;
    let end = rest.find("\n---")?;
    let front = &rest[..end];
    for line in front.lines() {
        if let Some((k, v)) = line.split_once(':')
            && k.trim() == field
        {
            let value = v.trim().trim_matches('"').trim_matches('\'').trim();
            return (!value.is_empty()).then(|| value.to_string());
        }
    }
    None
}

/// Parse Markdown links `[text](target)` from article body and classify each as
/// an article↔article or article↔file relation. `resolved` reflects whether the
/// target exists under the project (FR-009: dangling links are marked, not
/// fabricated). External `http(s)`/anchor links are ignored.
fn collect_link_relations(body: &str, project_dir: &Path, out: &mut Vec<Relation>) {
    let bytes = body.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b']'
            && i + 1 < bytes.len()
            && bytes[i + 1] == b'('
            && let Some(close) = body[i + 2..].find(')')
        {
            let target = body[i + 2..i + 2 + close].trim();
            if !target.is_empty()
                && !target.starts_with("http://")
                && !target.starts_with("https://")
                && !target.starts_with('#')
                && !target.starts_with("mailto:")
            {
                let clean = target.split(['#', '?']).next().unwrap_or(target);
                let resolved = project_dir.join(clean).exists();
                let relation_type =
                    if clean.ends_with(".md") { RelationType::ArticleToArticle } else { RelationType::ArticleToFile };
                out.push(Relation { relation_type, target: clean.to_string(), resolved });
            }
            i += 2 + close + 1;
            continue;
        }
        i += 1;
    }
}

/// Associate an article with same-named `prompt:`/`thinking:` siblings by the
/// repository's naming convention (`prompts/<name>.md`, `thinking/<name>.md`).
fn collect_sibling_relations(project_dir: &Path, article_identity: &str, out: &mut Vec<Relation>) {
    let name = article_identity.strip_prefix("article:").unwrap_or(article_identity);
    for (dir, prefix, relation_type) in [
        ("prompts", "prompt:", RelationType::ArticleToPrompt),
        ("thinking", "thinking:", RelationType::ArticleToThinking),
    ] {
        let path = project_dir.join(dir).join(format!("{name}.md"));
        if path.exists() {
            out.push(Relation { relation_type, target: format!("{prefix}{name}"), resolved: true });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::manifest::SearchDefaultMode;

    #[test]
    fn legacy_catalog_is_empty() {
        let config = ResolvedSourceConfig {
            backend: SourceBackend::Legacy,
            is_lance_active: false,
            is_marker_corrupt: false,
            activation_snapshot_id: None,
            storage_schema_version: None,
            chunk_tokens: 384,
            chunk_overlap: 48,
            fetch_max_bytes: 64 * 1024 * 1024,
            fetch_timeout_seconds: 30,
            fetch_max_redirects: 5,
            default_search_mode: SearchDefaultMode::Basic,
        };
        let catalog = SourceCatalog::discover(&config, Path::new("/tmp")).unwrap();
        assert!(!catalog.from_lance_primary);
        assert_eq!(catalog.backend, SourceBackend::Legacy);
    }

    #[test]
    fn discovery_exclusion_flags_plumbing_and_assets() {
        // Plumbing / computed cache.
        assert_eq!(discovery_exclusion("projects/alpha/mind-index.yaml"), Some("excluded"));
        assert_eq!(discovery_exclusion("minds.yaml"), Some("excluded"));
        assert_eq!(discovery_exclusion(".mind/registry.json"), Some("excluded"));
        assert_eq!(discovery_exclusion("projects/alpha/.mind-forge/enrichments/x.json"), Some("excluded"));
        // Build artifacts.
        assert_eq!(discovery_exclusion("projects/alpha/dist/article.html"), Some("excluded"));
        // Binary / assets.
        assert_eq!(discovery_exclusion("projects/alpha/assets/diagram.png"), Some("binary"));
        assert_eq!(discovery_exclusion("cover.PDF"), Some("binary"));
        // Eligible content.
        assert_eq!(discovery_exclusion("projects/alpha/outputs/2026-07/foo.md"), None);
        assert_eq!(discovery_exclusion("projects/alpha/sources/file/notes.md"), None);
    }

    #[test]
    fn lance_catalog_marks_primary() {
        let config = ResolvedSourceConfig {
            backend: SourceBackend::Lance,
            is_lance_active: true,
            is_marker_corrupt: false,
            activation_snapshot_id: Some("s".into()),
            storage_schema_version: Some("1".into()),
            chunk_tokens: 384,
            chunk_overlap: 48,
            fetch_max_bytes: 64 * 1024 * 1024,
            fetch_timeout_seconds: 30,
            fetch_max_redirects: 5,
            default_search_mode: SearchDefaultMode::Both,
        };
        let catalog = SourceCatalog::discover(&config, Path::new("/tmp")).unwrap();
        assert!(catalog.from_lance_primary);
        assert_eq!(catalog.backend, SourceBackend::Lance);
    }
}
