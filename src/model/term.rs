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
    Loose,
    #[default]
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
    /// Default-shaped correction used by `term new` and global equivalents:
    /// word match, required fix, standalone boundary, no pinyin.
    pub fn misrecognition(original: impl Into<String>, correct: impl Into<String>) -> Self {
        Self {
            original: original.into(),
            correct: correct.into(),
            r#match: MatchKind::Word,
            fix: FixKind::Required,
            boundary: Boundary::Standalone,
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
            // Boundary::Standalone only constrains ASCII word corrections; pinyin
            // and CJK originals ignore the boundary field entirely. Pinyin is
            // always "suggested" and dispatched through a separate scanner that
            // does not consult the boundary value.
            let is_ascii_word =
                c.original.bytes().all(|b| matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-'));
            if c.boundary == Boundary::Standalone && is_ascii_word && c.r#match != MatchKind::Pinyin {
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
    pub boundary_mode: &'static str,
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

// ── Correction selector & patch ─────────────────────────────────────────────

/// Identifies a single correction within a term by its `original` text.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)] // used by US3 correction subresource
pub struct CorrectionSelector {
    pub original: String,
}

/// Fields that can be updated on a single correction.
///
/// `pinyin` is `Option<Option<String>>`:
/// - `None` → not being changed
/// - `Some(None)` → clear the pinyin
/// - `Some(Some(v))` → set to `v`
#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)] // used by US3 correction subresource
pub struct CorrectionPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correct: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#match: Option<MatchKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<FixKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pinyin: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boundary: Option<Boundary>,
}

#[allow(dead_code)] // used by US3 implementation
impl CorrectionPatch {
    pub fn is_empty(&self) -> bool {
        self.correct.is_none()
            && self.r#match.is_none()
            && self.fix.is_none()
            && self.pinyin.is_none()
            && self.boundary.is_none()
    }
}

/// Report for a single-correction operation (add, update, remove, show).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)] // used by US3 correction subresource
pub struct CorrectionChangeReport {
    pub term: String,
    pub scope: String,
    pub correction: Correction,
    pub dry_run: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changes: Vec<String>,
}

/// Planned or completed side-effect during a term move.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)] // used by US4 term move
pub struct MoveSideEffect {
    pub action: String,
    pub scope: String,
    pub description: String,
}

/// Report for a term relocation between scopes.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)] // used by US4 term move
pub struct TermMoveReport {
    pub term: String,
    pub from_scope: String,
    pub to_scope: String,
    pub dry_run: bool,
    pub force: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub side_effects: Vec<MoveSideEffect>,
}

/// Targeted check boundary for term lint/fix operations.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)] // used by US6 targeted lint/fix
pub struct TermCheckTarget {
    pub target_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub article: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
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
            boundary: Boundary::Standalone,
            pinyin: None,
        };
        let yaml = serde_yaml::to_string(&c).unwrap();
        assert!(!yaml.contains("match:"), "default match:word must not write: {yaml}");
        assert!(!yaml.contains("fix:"), "default fix:required must not write: {yaml}");
        assert!(!yaml.contains("boundary:"), "default boundary:standalone must not write: {yaml}");
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
        assert_eq!(c.boundary, Boundary::Standalone);
        assert_eq!(c.pinyin, None);
    }

    #[test]
    fn boundary_default_is_standalone() {
        assert_eq!(Boundary::default(), Boundary::Standalone);
        assert!(Boundary::Standalone.is_default());
        assert!(!Boundary::Loose.is_default());
    }

    #[test]
    fn boundary_standalone_is_default_omitted_on_write() {
        let yaml = "original: aidc\ncorrect: AIDC\nboundary: standalone\n";
        let c: Correction = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(c.boundary, Boundary::Standalone);
        let written = serde_yaml::to_string(&c).unwrap();
        assert!(!written.contains("boundary:"), "standalone is default, must not serialize: {written}");
    }

    #[test]
    fn boundary_loose_is_explicit_and_serialized() {
        let c = Correction {
            original: "aidc".into(),
            correct: "AIDC".into(),
            r#match: MatchKind::Word,
            fix: FixKind::Required,
            boundary: Boundary::Loose,
            pinyin: None,
        };
        let written = serde_yaml::to_string(&c).unwrap();
        assert!(written.contains("boundary: loose"), "explicit loose must serialize: {written}");
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
    fn validate_corrections_allows_pinyin_with_standalone() {
        // Pinyin matches ignore the boundary field — standalone+pinyin passes validation.
        let terms = vec![term_with(vec![correction("kaifeidi", MatchKind::Pinyin, Boundary::Standalone)])];
        validate_corrections(&terms).expect("pinyin+standalone should pass validation");
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

    // ── CorrectionSelector / CorrectionPatch / reports ──────────────────────

    #[test]
    fn correction_selector_serialization() {
        let sel = CorrectionSelector { original: "typo".into() };
        let json = serde_json::to_value(&sel).unwrap();
        assert_eq!(json["original"], "typo");
    }

    #[test]
    fn correction_patch_is_empty_when_all_none() {
        let patch = CorrectionPatch::default();
        assert!(patch.is_empty());
    }

    #[test]
    fn correction_patch_is_not_empty_with_correct() {
        let patch = CorrectionPatch { correct: Some("fix".into()), ..Default::default() };
        assert!(!patch.is_empty());
    }

    #[test]
    fn correction_patch_is_not_empty_with_match_kind() {
        let patch = CorrectionPatch { r#match: Some(MatchKind::Substring), ..Default::default() };
        assert!(!patch.is_empty());
    }

    #[test]
    fn correction_patch_serialization_skips_none() {
        let patch = CorrectionPatch { r#match: Some(MatchKind::Pinyin), ..Default::default() };
        let json = serde_json::to_value(&patch).unwrap();
        assert!(json.get("correct").is_none());
        assert_eq!(json["match"], "pinyin");
    }

    #[test]
    fn correction_patch_pinyin_clear() {
        let patch = CorrectionPatch { pinyin: Some(None), ..Default::default() };
        let json = serde_json::to_value(&patch).unwrap();
        assert_eq!(json["pinyin"], serde_json::Value::Null);
    }

    #[test]
    fn correction_change_report_serialization() {
        let report = CorrectionChangeReport {
            term: "RAG".into(),
            scope: "project".into(),
            correction: Correction {
                original: "rag".into(),
                correct: "RAG".into(),
                r#match: MatchKind::Word,
                fix: FixKind::Required,
                boundary: Boundary::Standalone,
                pinyin: None,
            },
            dry_run: false,
            changes: vec!["correct: rag → RAG".into()],
        };
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["term"], "RAG");
        assert_eq!(json["scope"], "project");
        assert_eq!(json["dry_run"], false);
        assert_eq!(json["changes"][0], "correct: rag → RAG");
    }

    #[test]
    fn term_move_report_serialization() {
        let report = TermMoveReport {
            term: "RAG".into(),
            from_scope: "project".into(),
            to_scope: "global".into(),
            dry_run: true,
            force: false,
            side_effects: vec![],
        };
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["term"], "RAG");
        assert_eq!(json["from_scope"], "project");
        assert_eq!(json["to_scope"], "global");
        assert_eq!(json["dry_run"], true);
        assert_eq!(json["force"], false);
    }

    #[test]
    fn move_side_effect_serialization() {
        let se = MoveSideEffect {
            action: "remove".into(),
            scope: "project".into(),
            description: "remove RAG from project alpha".into(),
        };
        let json = serde_json::to_value(&se).unwrap();
        assert_eq!(json["action"], "remove");
        assert_eq!(json["scope"], "project");
    }

    #[test]
    fn term_check_target_serialization() {
        let target = TermCheckTarget {
            target_type: "article".into(),
            project: Some("alpha".into()),
            article: Some("weekly-note".into()),
            path: None,
        };
        let json = serde_json::to_value(&target).unwrap();
        assert_eq!(json["target_type"], "article");
        assert_eq!(json["project"], "alpha");
        assert_eq!(json["article"], "weekly-note");
        assert!(json.get("path").is_none());
    }

    #[test]
    fn term_check_target_file_type() {
        let target = TermCheckTarget {
            target_type: "file".into(),
            project: None,
            article: None,
            path: Some("docs/draft.md".into()),
        };
        let json = serde_json::to_value(&target).unwrap();
        assert_eq!(json["target_type"], "file");
        assert_eq!(json["path"], "docs/draft.md");
    }
}
