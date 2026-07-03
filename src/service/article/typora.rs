use std::path::Path;

/// Returns true unless the Typora plugin is explicitly disabled.
pub fn effective_typora_enabled(plugins: Option<&crate::model::config::PluginsConfig>) -> bool {
    plugins.and_then(|p| p.typora_front_matter.as_ref()).and_then(|t| t.enabled).unwrap_or(true)
}

/// Compute the value for `typora-copy-images-to`.
///
/// - Absolute `assets` → emitted unchanged.
/// - Relative `assets` → POSIX relative path from `file_dir` to
///   `project_path.join(assets)`.
pub fn compute_typora_assets_path(project_path: &Path, assets: &str, file_dir: &Path) -> String {
    if assets.starts_with('/') {
        return assets.to_string();
    }
    let assets_abs = project_path.join(assets);
    relative_path_from(file_dir, &assets_abs)
}

/// POSIX-style relative path from `from_dir` to `to_dir`.
fn relative_path_from(from_dir: &Path, to_dir: &Path) -> String {
    // Canonicalize both if possible; fall back to the input paths.
    let from = from_dir.canonicalize().unwrap_or_else(|_| from_dir.to_path_buf());
    let to = to_dir.canonicalize().unwrap_or_else(|_| to_dir.to_path_buf());

    let mut from_components: Vec<&std::ffi::OsStr> = from
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(p) => Some(p),
            _ => None,
        })
        .collect();
    let mut to_components: Vec<&std::ffi::OsStr> = to
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(p) => Some(p),
            _ => None,
        })
        .collect();

    // Strip common prefix
    let common = from_components.iter().zip(to_components.iter()).take_while(|(f, t)| f == t).count();
    from_components.drain(..common);
    to_components.drain(..common);

    let mut result = Vec::new();
    for _ in &from_components {
        result.push("..".to_string());
    }
    for c in &to_components {
        result.push(c.to_string_lossy().to_string());
    }

    if result.is_empty() {
        ".".to_string()
    } else {
        result.join("/")
    }
}

/// Inject or merge `typora-copy-images-to` into an initial YAML front-matter block.
///
/// - No front matter → prepends a new `---` block.
/// - Existing front matter with `typora-copy-images-to` → returns unchanged.
/// - Existing front matter without the key → inserts the key before the closing `---`.
pub fn inject_typora_front_matter(content: &str, assets_path: &str) -> String {
    let key_line = format!("typora-copy-images-to: {}", assets_path);

    if let Some((front, body, eol)) = split_initial_yaml_front_matter(content) {
        if front.lines().any(is_typora_copy_images_to_line) {
            return content.to_string();
        }

        let mut merged = String::new();
        merged.push_str("---");
        merged.push_str(eol);
        merged.push_str(front);
        if !front.is_empty() && !front.ends_with('\n') {
            merged.push_str(eol);
        }
        merged.push_str(&key_line);
        merged.push_str(eol);
        merged.push_str("---");
        merged.push_str(eol);
        merged.push_str(body);
        return merged;
    }

    // No front matter block
    format!("---\n{}\n---\n\n{}", key_line, content)
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
    let line = line.trim_start();
    line.starts_with("typora-copy-images-to:")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── T011: Typora front-matter helper unit tests ──

    #[test]
    fn inject_no_front_matter_prepend() {
        let content = "# Title\n\nBody\n";
        let result = inject_typora_front_matter(content, "../assets");
        assert!(result.starts_with("---\n"));
        assert!(result.contains("typora-copy-images-to: ../assets\n"));
        assert!(result.contains("# Title"));
    }

    #[test]
    fn inject_existing_front_matter_merge() {
        let content = "---\ntitle: Test\n---\n# Body\n";
        let result = inject_typora_front_matter(content, "../assets");
        // Should have exactly one --- block
        assert_eq!(result.matches("---").count(), 2, "should have exactly one opening and one closing ---");
        assert!(result.contains("typora-copy-images-to: ../assets\n"));
        assert!(result.contains("title: Test\n"));
        assert!(result.contains("# Body"));
    }

    #[test]
    fn inject_existing_typora_value_preserved() {
        let content = "---\ntitle: Test\ntypora-copy-images-to: ../../media\n---\n# Body\n";
        let result = inject_typora_front_matter(content, "../assets");
        // The existing value should be preserved (not our injected arg)
        assert!(result.contains("typora-copy-images-to: ../../media\n"));
        assert!(!result.contains("typora-copy-images-to: ../assets\n"));
    }

    #[test]
    fn inject_existing_front_matter_with_crlf_merge() {
        let content = "---\r\ntitle: Test\r\n---\r\n# Body\r\n";
        let result = inject_typora_front_matter(content, "../assets");
        assert!(result.starts_with("---\r\n"));
        assert!(result.contains("title: Test\r\n"));
        assert!(result.contains("typora-copy-images-to: ../assets\r\n"));
        assert!(result.contains("---\r\n# Body\r\n"));
    }

    #[test]
    fn inject_comment_mentioning_typora_key_does_not_count_as_existing_value() {
        let content = "---\n# typora-copy-images-to: old\ntitle: Test\n---\n# Body\n";
        let result = inject_typora_front_matter(content, "../assets");
        assert!(result.contains("# typora-copy-images-to: old\n"));
        assert!(result.contains("typora-copy-images-to: ../assets\n"));
    }

    #[test]
    fn compute_path_absolute() {
        let result = compute_typora_assets_path(
            Path::new("/Users/me/proj"),
            "/static/images",
            Path::new("/Users/me/proj/docs/post"),
        );
        assert_eq!(result, "/static/images");
    }

    #[test]
    fn compute_path_root_relative() {
        // For a file in docs/, assets as "assets" relative to project root = ../assets from docs/
        let tmp = tempfile::tempdir().unwrap();
        let assets_dir = tmp.path().join("assets");
        std::fs::create_dir_all(&assets_dir).unwrap();
        let docs_dir = tmp.path().join("docs");
        std::fs::create_dir_all(&docs_dir).unwrap();
        let result = compute_typora_assets_path(tmp.path(), "assets", &docs_dir);
        assert_eq!(result, "../assets");
    }

    #[test]
    fn compute_path_directory_article_relative() {
        let tmp = tempfile::tempdir().unwrap();
        let assets_dir = tmp.path().join("assets");
        std::fs::create_dir_all(&assets_dir).unwrap();
        let post_dir = tmp.path().join("docs/my-post");
        std::fs::create_dir_all(&post_dir).unwrap();
        let result = compute_typora_assets_path(tmp.path(), "assets", &post_dir);
        assert_eq!(result, "../../assets");
    }

    #[test]
    fn compute_path_single_file_relative() {
        let tmp = tempfile::tempdir().unwrap();
        let assets_dir = tmp.path().join("assets");
        std::fs::create_dir_all(&assets_dir).unwrap();
        let docs_dir = tmp.path().join("docs");
        std::fs::create_dir_all(&docs_dir).unwrap();
        let result = compute_typora_assets_path(tmp.path(), "assets", &docs_dir);
        assert_eq!(result, "../assets");
    }

    #[test]
    fn effective_enabled_default() {
        assert!(effective_typora_enabled(None));
    }

    #[test]
    fn effective_enabled_missing_plugin() {
        let plugins = crate::model::config::PluginsConfig::default();
        assert!(effective_typora_enabled(Some(&plugins)));
    }

    #[test]
    fn effective_enabled_explicit_true() {
        let plugins = crate::model::config::PluginsConfig {
            typora_front_matter: Some(crate::model::config::TyporaFrontMatterPluginConfig {
                enabled: Some(true),
                extra: serde_yaml::Mapping::new(),
            }),
            ..Default::default()
        };
        assert!(effective_typora_enabled(Some(&plugins)));
    }

    #[test]
    fn effective_enabled_explicit_false() {
        let plugins = crate::model::config::PluginsConfig {
            typora_front_matter: Some(crate::model::config::TyporaFrontMatterPluginConfig {
                enabled: Some(false),
                extra: serde_yaml::Mapping::new(),
            }),
            ..Default::default()
        };
        assert!(!effective_typora_enabled(Some(&plugins)));
    }
}
