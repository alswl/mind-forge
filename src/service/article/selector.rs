use std::collections::BTreeSet;
use std::path::Path;

use crate::error::{MfError, Result};
use crate::model::article::Article;
use crate::service::index::{self, article_output_stem};

/// Resolve a user-supplied article selector to the unique `article_path` it
/// identifies (spec 079 US3/US6, FR-007/FR-008/FR-009, data-model.md §2).
///
/// Collects *all* candidates matching any of four exact forms — full
/// `article_path`, `article_path` with `.md` stripped, bare slug (via
/// [`article_output_stem`]), or exact `title` — then decides:
///
/// - 0 candidates → `MfError::not_found`, pointing at `mf article list`.
/// - more than 1 candidate → `MfError::usage` (ambiguous), listing every
///   candidate `article_path` rather than guessing.
/// - exactly 1 → that entry's `article_path`.
///
/// Deliberately does **not** fall back to `mf article show`'s `contains`
/// substring match — a write operation silently guessing among substring
/// matches (`remove 2026-09` matching `2026-09-monthly`) is dangerous.
/// `mf article show` itself is unaffected; it keeps its own looser resolver.
pub fn resolve_selector(project_path: &Path, selector: &str) -> Result<String> {
    let idx = index::load(project_path)?;
    let articles: &[Article] = idx.articles.as_deref().unwrap_or(&[]);

    let bare_slug = article_output_stem(selector);
    let selector_without_md = selector.strip_suffix(".md").unwrap_or(selector);

    // A BTreeSet dedups an entry that matches more than one form at once
    // (e.g. a single-file article whose `article_path` bare slug equals the
    // selector already collected via the full-path form) so it isn't
    // reported as two separate ambiguous candidates.
    let mut candidates: BTreeSet<&str> = BTreeSet::new();
    for a in articles {
        let path = a.article_path.as_str();
        let path_without_md = path.strip_suffix(".md").unwrap_or(path);
        if path == selector
            || path_without_md == selector_without_md
            || article_output_stem(path) == bare_slug
            || a.title == selector
        {
            candidates.insert(path);
        }
    }

    match candidates.len() {
        0 => Err(MfError::not_found(
            format!("article '{selector}' not found"),
            Some("use `mf article list --project <project>` to see available articles".to_string()),
        )),
        1 => Ok(candidates.into_iter().next().expect("len checked").to_string()),
        _ => {
            let list = candidates.iter().map(|c| format!("'{c}'")).collect::<Vec<_>>().join(", ");
            Err(MfError::usage(
                format!("selector '{selector}' is ambiguous: matches {list}"),
                Some("pass the full article path to disambiguate".to_string()),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::article::{ArticleStatus, ArticleType};

    fn article(title: &str, article_path: &str) -> Article {
        Article {
            title: title.to_string(),
            project: "alpha".to_string(),
            article_type: ArticleType::Blank,
            article_path: article_path.to_string(),
            status: ArticleStatus::Draft,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            template_origin: None,
        }
    }

    fn write_index(project_path: &Path, articles: Vec<Article>) {
        use crate::model::index::IndexFile;
        std::fs::create_dir_all(project_path).unwrap();
        let idx = IndexFile { articles: Some(articles), ..IndexFile::create_default() };
        index::save(project_path, &idx).unwrap();
    }

    fn tmp_project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("mind.yaml"), "schema: '1'\n").unwrap();
        dir
    }

    #[test]
    fn resolves_full_article_path() {
        let dir = tmp_project();
        write_index(dir.path(), vec![article("Monthly", "docs/2026-09-monthly.md")]);
        assert_eq!(resolve_selector(dir.path(), "docs/2026-09-monthly.md").unwrap(), "docs/2026-09-monthly.md");
    }

    #[test]
    fn resolves_article_path_without_md_suffix() {
        let dir = tmp_project();
        write_index(dir.path(), vec![article("Monthly", "docs/2026-09-monthly.md")]);
        assert_eq!(resolve_selector(dir.path(), "docs/2026-09-monthly").unwrap(), "docs/2026-09-monthly.md");
    }

    #[test]
    fn resolves_bare_slug() {
        let dir = tmp_project();
        write_index(dir.path(), vec![article("Monthly", "docs/2026-09-monthly.md")]);
        assert_eq!(resolve_selector(dir.path(), "2026-09-monthly").unwrap(), "docs/2026-09-monthly.md");
    }

    #[test]
    fn resolves_bare_slug_for_directory_article() {
        let dir = tmp_project();
        write_index(dir.path(), vec![article("Monthly", "docs/2026-09-monthly")]);
        assert_eq!(resolve_selector(dir.path(), "2026-09-monthly").unwrap(), "docs/2026-09-monthly");
    }

    #[test]
    fn resolves_exact_title() {
        let dir = tmp_project();
        write_index(dir.path(), vec![article("Monthly Report", "docs/2026-09-monthly.md")]);
        assert_eq!(resolve_selector(dir.path(), "Monthly Report").unwrap(), "docs/2026-09-monthly.md");
    }

    #[test]
    fn not_found_reports_hint() {
        let dir = tmp_project();
        write_index(dir.path(), vec![article("Monthly", "docs/2026-09-monthly.md")]);
        let err = resolve_selector(dir.path(), "nonexistent").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("not found"), "{msg}");
    }

    #[test]
    fn ambiguous_when_directory_and_single_file_share_a_slug() {
        // `docs/dup` (a directory article) and `docs/dup.md` (a single-file
        // article at the same path minus `.md`) cannot coexist in a real
        // index: `article_key` — the index's own map key — strips `.md` and
        // any trailing `/` but *not* the `docs/`/`outputs/` prefix, so both
        // forms reduce to the identical key `docs/dup` and one would
        // silently overwrite the other on any index rebuild. The
        // representable version of this ambiguity is a directory article
        // under `docs/` and a single-file *generated* article under
        // `outputs/` that happen to share a bare slug — distinct map keys,
        // same `article_output_stem`.
        let dir = tmp_project();
        write_index(dir.path(), vec![article("Dup Dir", "docs/dup"), article("Dup Generated", "outputs/dup.md")]);
        let err = resolve_selector(dir.path(), "dup").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("ambiguous"), "{msg}");
        assert!(msg.contains("docs/dup") && msg.contains("outputs/dup.md"), "{msg}");
    }

    #[test]
    fn contains_substring_does_not_match() {
        let dir = tmp_project();
        write_index(dir.path(), vec![article("Monthly", "docs/2026-09-monthly.md")]);
        let err = resolve_selector(dir.path(), "2026-09").unwrap_err();
        assert!(err.to_string().contains("not found"));
    }
}
