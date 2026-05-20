pub const SCHEMA_VERSION: &str = "1";
pub const PROJECTS_DIR: &str = "projects";

pub const BUILD_OUTPUT_DIR: &str = "outputs";
pub const BUILD_OUTPUT_PATH_PREFIX: &str = "outputs/";
pub const DEFAULT_BUILD_FORMAT: &str = "md";
pub const MARKDOWN_EXTENSION: &str = "md";

pub const DOCS_DIR: &str = "docs";
pub const DOCS_PATH_PREFIX: &str = "docs/";
pub const SOURCES_DIR: &str = "sources";
pub const ASSETS_DIR: &str = "assets";
pub const TEMPLATES_DIR: &str = "templates";
pub const ARCHIVE_DIR: &str = "_archived";

pub const REQUIRED_PROJECT_DIRS: &[&str] = &[DOCS_DIR, SOURCES_DIR, ASSETS_DIR];

/// Default layout category values.
pub const LAYOUT_ARTICLES_DEFAULT: &str = DOCS_DIR;
pub const LAYOUT_SOURCES_DEFAULT: &str = SOURCES_DIR;
pub const LAYOUT_ASSETS_DEFAULT: &str = ASSETS_DIR;
pub const LAYOUT_TEMPLATES_DEFAULT: &str = TEMPLATES_DIR;
pub const LAYOUT_BUILD_OUTPUT_DEFAULT: &str = BUILD_OUTPUT_DIR;

/// Maximum directory depth when searching upward for a minds.yaml repo root.
pub const MAX_REPO_SEARCH_DEPTH: usize = 50;
