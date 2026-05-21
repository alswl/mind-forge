use serde::Serialize;

/// What a referencing object is (used inside `Reference`).
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)] // variants consumed by US3 move
pub enum ObjectKind {
    Term,
    Source,
    Asset,
    Article,
    Project,
}

/// How a reference points to the target.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)] // Metadata consumed by US3 move
pub enum ReferenceKind {
    Mention,
    Index,
    Metadata,
}

/// A single reference from one primary object to another.
#[derive(Debug, Clone, Serialize)]
pub struct Reference {
    pub from_kind: ObjectKind,
    /// Stable identifier of the referencing object (article title,
    /// term primary name, source path, asset path, project name).
    pub from_id: String,
    /// File path containing the reference, relative to the project (or repo
    /// for project-scoped references). `null` when only the index entry
    /// referenced the target.
    pub from_path: Option<String>,
    /// Line number (1-based) in `from_path` when known.
    pub line: Option<usize>,
    pub kind: ReferenceKind,
}

/// One filesystem or YAML change the verb would make.
#[derive(Debug, Clone, Serialize)]
pub struct PlannedChange {
    pub op: PlannedOp,
    /// Path relative to the repo root.
    pub path: String,
    /// Old value when meaningful (e.g., previous YAML key, old filename).
    pub old: Option<String>,
    /// New value when meaningful (e.g., new filename, new YAML key).
    pub new: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)] // RenameDir consumed by US3 move
pub enum PlannedOp {
    RemoveFile,
    RemoveDir,
    RenameFile,
    RenameDir,
    UpdateYaml,
    RefreshIndex,
}

/// Where a scoped object lives, used by `term move` and the report
/// `before`/`after` fields.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ScopeRef {
    /// Name of the owning project, when scope is project-local.
    pub project: Option<String>,
    /// True when scope is the global repo-level pool (terms only today).
    pub global: bool,
}

// ── Deterministic ordering helpers ───────────────────────────────────────

impl Reference {
    /// Sort key: `(from_path, from_id)` for deterministic output.
    #[allow(dead_code)] // consumed by US3 move
    pub fn sort_key(&self) -> (&Option<String>, &str) {
        (&self.from_path, &self.from_id)
    }
}

#[allow(dead_code)] // consumed by US3 move
pub fn sort_references(refs: &mut [Reference]) {
    refs.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
}

impl PlannedChange {
    /// Sort key: `(path, op)` for deterministic output.
    #[allow(dead_code)] // consumed by US3 move
    pub fn sort_key(&self) -> (&str, &PlannedOp) {
        (self.path.as_str(), &self.op)
    }
}

#[allow(dead_code)] // consumed by US3 move
pub fn sort_planned_changes(changes: &mut [PlannedChange]) {
    changes.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_sorting_deterministic() {
        let mut refs = vec![
            Reference {
                from_kind: ObjectKind::Article,
                from_id: "zeta".into(),
                from_path: Some("docs/b.md".into()),
                line: None,
                kind: ReferenceKind::Mention,
            },
            Reference {
                from_kind: ObjectKind::Article,
                from_id: "alpha".into(),
                from_path: Some("docs/a.md".into()),
                line: Some(42),
                kind: ReferenceKind::Mention,
            },
        ];
        sort_references(&mut refs);
        assert_eq!(refs[0].from_id, "alpha");
        assert_eq!(refs[1].from_id, "zeta");
    }

    #[test]
    fn planned_change_sorting_deterministic() {
        let mut changes = vec![
            PlannedChange {
                op: PlannedOp::RefreshIndex,
                path: "projects/foo/mind-index.yaml".into(),
                old: None,
                new: None,
            },
            PlannedChange {
                op: PlannedOp::UpdateYaml,
                path: "projects/foo/terms.yaml".into(),
                old: Some("TLA".into()),
                new: None,
            },
        ];
        sort_planned_changes(&mut changes);
        assert_eq!(changes[0].path, "projects/foo/mind-index.yaml");
        assert_eq!(changes[1].path, "projects/foo/terms.yaml");
    }

    #[test]
    fn scope_ref_serialization() {
        let scope = ScopeRef { project: Some("alpha".into()), global: false };
        let json = serde_json::to_value(&scope).unwrap();
        assert_eq!(json["project"], "alpha");
        assert_eq!(json["global"], false);

        let global_scope = ScopeRef { project: None, global: true };
        let json = serde_json::to_value(&global_scope).unwrap();
        assert_eq!(json["project"], serde_json::Value::Null);
        assert_eq!(json["global"], true);
    }
}
