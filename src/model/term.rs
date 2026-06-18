use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MatchKind {
    #[default]
    Word,
    Substring,
    Pinyin,
}

impl MatchKind {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FixKind {
    #[default]
    Required,
    Suggested,
}

impl FixKind {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Correction {
    pub original: String,
    pub correct: String,
    #[serde(default, skip_serializing_if = "MatchKind::is_default")]
    pub r#match: MatchKind,
    #[serde(default, skip_serializing_if = "FixKind::is_default")]
    pub fix: FixKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinyin: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Term {
    pub term: String,
    pub definition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    pub replacement_eligible: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<CandidateTerm>,
    pub match_kind: String,
    pub fix_kind: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct CandidateTerm {
    pub term: String,
    pub correct: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct TermLintFailure {
    pub path: String,
    pub reason: String,
}

// ── Lifecycle reports ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct TermRemoveReport {
    pub verb: String,
    pub kind: String,
    pub before: TermIdentity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<TermIdentity>,
    #[serde(default)]
    pub references: Vec<crate::model::lifecycle::Reference>,
    #[serde(default)]
    pub side_effects: Vec<crate::model::lifecycle::PlannedChange>,
    pub force: bool,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct TermIdentity {
    pub name: String,
    pub scope: crate::model::lifecycle::ScopeRef,
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
    pub would_apply_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correction_defaults_not_serialized() {
        let c = Correction {
            original: "mindrepo".into(),
            correct: "Mind Repo".into(),
            r#match: MatchKind::Word,
            fix: FixKind::Required,
            pinyin: None,
        };
        let yaml = serde_yaml::to_string(&c).unwrap();
        assert!(!yaml.contains("match:"), "default match:word must not write: {yaml}");
        assert!(!yaml.contains("fix:"), "default fix:required must not write: {yaml}");
        assert!(!yaml.contains("pinyin:"), "None pinyin must not write: {yaml}");
    }

    #[test]
    fn correction_explicit_values_roundtrip() {
        let c = Correction {
            original: "凯飞迪".into(),
            correct: "凯飞迪".into(),
            r#match: MatchKind::Pinyin,
            fix: FixKind::Suggested,
            pinyin: Some("kai-fei-di".into()),
        };
        let yaml = serde_yaml::to_string(&c).unwrap();
        let roundtripped: Correction = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(roundtripped.r#match, MatchKind::Pinyin);
        assert_eq!(roundtripped.fix, FixKind::Suggested);
        assert_eq!(roundtripped.pinyin.as_deref(), Some("kai-fei-di"));
    }

    #[test]
    fn old_yaml_without_new_fields_loads_defaults() {
        let old = "original: mindrepo\ncorrect: Mind Repo\n";
        let c: Correction = serde_yaml::from_str(old).unwrap();
        assert_eq!(c.original, "mindrepo");
        assert_eq!(c.correct, "Mind Repo");
        assert_eq!(c.r#match, MatchKind::Word);
        assert_eq!(c.fix, FixKind::Required);
        assert_eq!(c.pinyin, None);
    }

    #[test]
    fn match_kind_default_is_word() {
        assert_eq!(MatchKind::default(), MatchKind::Word);
        assert!(MatchKind::Word.is_default());
        assert!(!MatchKind::Substring.is_default());
        assert!(!MatchKind::Pinyin.is_default());
    }

    #[test]
    fn fix_kind_default_is_required() {
        assert_eq!(FixKind::default(), FixKind::Required);
        assert!(FixKind::Required.is_default());
        assert!(!FixKind::Suggested.is_default());
    }

    #[test]
    fn match_kind_invalid_variant_fails() {
        let yaml = "original: foo\ncorrect: bar\nmatch: bogus\n";
        let result: Result<Correction, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err(), "unknown match variant must fail");
    }
}
