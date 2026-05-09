use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Correction {
    pub original: String,
    pub correct: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Term {
    pub term: String,
    pub definition: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub corrections: Vec<Correction>,
}

// ── View models (012-term-core) ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct TermFinding {
    pub path: String,
    pub line: u32,
    pub column: u32,
    pub original: String,
    pub correct: String,
    pub term: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct TermLintFailure {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct TermLintReport {
    pub findings: Vec<TermFinding>,
    pub scanned_files: u64,
    pub skipped_files: Vec<String>,
    pub fixed_count: u64,
    pub modified_files: Vec<String>,
    pub failures: Vec<TermLintFailure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub would_fix_count: Option<u64>,
}
