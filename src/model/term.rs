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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Boundary {
    #[default]
    Loose,
    Standalone,
}

impl Boundary {
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
    #[serde(default, skip_serializing_if = "Boundary::is_default")]
    pub boundary: Boundary,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinyin: Option<String>,
}

impl Correction {
    /// Default-shaped correction used by `term new`, `term add`, and global
    /// equivalents: word match, required fix, loose boundary, no pinyin.
    pub fn misrecognition(original: impl Into<String>, correct: impl Into<String>) -> Self {
        Self {
            original: original.into(),
            correct: correct.into(),
            r#match: MatchKind::Word,
            fix: FixKind::Required,
            boundary: Boundary::Loose,
            pinyin: None,
        }
    }
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

/// Validate cross-field invariants on every correction. Returns a user-facing
/// message on the first violation. Called by both the project-scoped index
/// loader (`mind-index.yaml`) and the global terms loader (`minds-terms.yaml`)
/// so the same rules apply regardless of scope.
pub fn validate_corrections(terms: &[Term]) -> std::result::Result<(), String> {
    for term in terms {
        for c in &term.corrections {
            if c.boundary == Boundary::Standalone {
                if c.r#match != MatchKind::Word {
                    let kind = match c.r#match {
                        MatchKind::Substring => "substring",
                        MatchKind::Pinyin => "pinyin",
                        MatchKind::Word => unreachable!(),
                    };
                    return Err(format!(
                        "boundary: standalone is only valid with match: word (correction '{}' uses match: {})",
                        c.original, kind
                    ));
                }
                let bytes = c.original.as_bytes();
                if bytes.first().is_some_and(|b| *b == b'-' || *b == b'_')
                    || bytes.last().is_some_and(|b| *b == b'-' || *b == b'_')
                {
                    return Err(format!(
                        "boundary: standalone cannot apply to identifier-character edges (correction '{}')",
                        c.original
                    ));
                }
            }
        }
    }
    Ok(())
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
    pub match_kind: MatchKind,
    pub fix_kind: FixKind,
    pub boundary: Boundary,
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
            boundary: Boundary::Loose,
            pinyin: None,
        };
        let yaml = serde_yaml::to_string(&c).unwrap();
        assert!(!yaml.contains("match:"), "default match:word must not write: {yaml}");
        assert!(!yaml.contains("fix:"), "default fix:required must not write: {yaml}");
        assert!(!yaml.contains("boundary:"), "default boundary:loose must not write: {yaml}");
        assert!(!yaml.contains("pinyin:"), "None pinyin must not write: {yaml}");
    }

    #[test]
    fn correction_explicit_values_roundtrip() {
        let c = Correction {
            original: "凯飞迪".into(),
            correct: "凯飞迪".into(),
            r#match: MatchKind::Pinyin,
            fix: FixKind::Suggested,
            boundary: Boundary::Loose,
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
        assert_eq!(c.boundary, Boundary::Loose);
        assert_eq!(c.pinyin, None);
    }

    #[test]
    fn boundary_default_is_loose() {
        assert_eq!(Boundary::default(), Boundary::Loose);
        assert!(Boundary::Loose.is_default());
        assert!(!Boundary::Standalone.is_default());
    }

    #[test]
    fn boundary_standalone_roundtrip_preserves_value() {
        let yaml = "original: aidc\ncorrect: AIDC\nboundary: standalone\n";
        let c: Correction = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(c.boundary, Boundary::Standalone);
        let written = serde_yaml::to_string(&c).unwrap();
        assert!(written.contains("boundary: standalone"), "standalone must serialize: {written}");
    }

    #[test]
    fn boundary_loose_not_serialized_even_when_explicit() {
        let c = Correction {
            original: "aidc".into(),
            correct: "AIDC".into(),
            r#match: MatchKind::Word,
            fix: FixKind::Required,
            boundary: Boundary::Loose,
            pinyin: None,
        };
        let written = serde_yaml::to_string(&c).unwrap();
        assert!(!written.contains("boundary:"), "explicit loose must still be omitted: {written}");
    }

    #[test]
    fn boundary_invalid_variant_fails() {
        let yaml = "original: foo\ncorrect: bar\nboundary: bogus\n";
        let result: Result<Correction, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err(), "unknown boundary variant must fail");
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

    // ── validate_corrections ────────────────────────────────────────────────

    fn term_with(corrections: Vec<Correction>) -> Term {
        Term {
            term: "TEST".into(),
            definition: None,
            description: None,
            confidence: None,
            aliases: vec![],
            tags: vec![],
            corrections,
        }
    }

    fn correction(original: &str, m: MatchKind, b: Boundary) -> Correction {
        Correction {
            original: original.into(),
            correct: "X".into(),
            r#match: m,
            fix: FixKind::Required,
            boundary: b,
            pinyin: None,
        }
    }

    #[test]
    fn validate_corrections_accepts_loose_with_any_match_kind() {
        let terms = vec![term_with(vec![
            correction("a", MatchKind::Word, Boundary::Loose),
            correction("b", MatchKind::Substring, Boundary::Loose),
            correction("c", MatchKind::Pinyin, Boundary::Loose),
        ])];
        assert!(validate_corrections(&terms).is_ok());
    }

    #[test]
    fn validate_corrections_rejects_standalone_with_substring() {
        let terms = vec![term_with(vec![correction("aidc", MatchKind::Substring, Boundary::Standalone)])];
        let err = validate_corrections(&terms).unwrap_err();
        assert!(err.contains("standalone is only valid with match: word"), "got: {err}");
        assert!(err.contains("aidc"), "got: {err}");
    }

    #[test]
    fn validate_corrections_rejects_standalone_with_pinyin() {
        let terms = vec![term_with(vec![correction("凯飞迪", MatchKind::Pinyin, Boundary::Standalone)])];
        let err = validate_corrections(&terms).unwrap_err();
        assert!(err.contains("standalone is only valid with match: word"), "got: {err}");
        assert!(err.contains("凯飞迪"), "got: {err}");
        assert!(err.contains("pinyin"), "got: {err}");
    }

    #[test]
    fn validate_corrections_rejects_edge_hyphen() {
        let terms = vec![term_with(vec![correction("aidc-", MatchKind::Word, Boundary::Standalone)])];
        let err = validate_corrections(&terms).unwrap_err();
        assert!(err.contains("identifier-character edges"), "got: {err}");
    }

    #[test]
    fn validate_corrections_rejects_edge_underscore_leading() {
        let terms = vec![term_with(vec![correction("_aidc", MatchKind::Word, Boundary::Standalone)])];
        let err = validate_corrections(&terms).unwrap_err();
        assert!(err.contains("identifier-character edges"), "got: {err}");
    }
}
