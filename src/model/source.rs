use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// SourceKind — source channel/origin (mind primary)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Yuque,
    Dima,
    Meeting,
    Misc,
}

impl SourceKind {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Yuque => "yuque",
            Self::Dima => "dima",
            Self::Meeting => "meeting",
            Self::Misc => "misc",
        }
    }
}

impl std::fmt::Display for SourceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// FileKind — file format type (prev. SourceKind in mf)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    Auto,
    Pdf,
    Rss,
    Web,
    File,
}

impl FileKind {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Pdf => "pdf",
            Self::File => "file",
            Self::Rss => "rss",
            Self::Web => "web",
        }
    }
}

impl std::fmt::Display for FileKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Source {
    pub name: String,
    #[serde(rename = "type", alias = "file_kind")]
    pub kind: FileKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_kind: Option<SourceKind>,
    pub url: Option<String>,
    pub path: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub added_at: String,
    pub updated_at: String,
}

// ---------------------------------------------------------------------------
// T003: SourceIndexEntry — used by `mf source index` / `mf source clean`
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SourceIndexEntry {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: FileKind,
    pub path: String,
}

// ---------------------------------------------------------------------------
// T004: SourceIndexReport — used by `mf source index` / `mf source clean`
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SourceIndexReport {
    pub added: Vec<SourceIndexEntry>,
    pub removed: Vec<SourceIndexEntry>,
    pub kept_count: u64,
    pub dry_run: bool,
}

// ---------------------------------------------------------------------------
// T005: SourceRemoveReport — used by `mf source remove`
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SourceRemoveReport {
    #[serde(flatten)]
    pub source: Source,
    pub file_deleted: bool,
}
