//! Durable transaction intents for cross-store Project lifecycle operations.
//!
//! Project create/import/rename/archive/remove span both the root filesystem
//! Project catalog and the LanceDB primary registrations. Each operation
//! records a durable JSON intent under `.mind/source/advanced/transactions/`
//! so that crashes mid-operation can be recovered deterministically.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::{MfError, Result};
use crate::model::source_advanced::{AdvancedMutationIntent, MutationOperation, MutationPhase};

const TRANSACTIONS_DIR: &str = "transactions";

/// Build the path to the transactions directory.
pub fn transactions_dir(advanced_dir: &Path) -> PathBuf {
    advanced_dir.join(TRANSACTIONS_DIR)
}

/// Build the path for a specific transaction record.
pub fn transaction_path(advanced_dir: &Path, transaction_id: &str) -> PathBuf {
    transactions_dir(advanced_dir).join(format!("{transaction_id}.json"))
}

// ── Intent CRUD ────────────────────────────────────────────────────────────

/// Create and persist a new mutation intent in the `prepared` phase.
pub fn create_intent(advanced_dir: &Path, intent: &AdvancedMutationIntent) -> Result<()> {
    let dir = transactions_dir(advanced_dir);
    fs::create_dir_all(&dir)?;

    let path = transaction_path(advanced_dir, &intent.transaction_id);
    let json = serde_json::to_string_pretty(intent).map_err(MfError::Json)?;
    let mut f = fs::File::create(&path)?;
    f.write_all(json.as_bytes())?;
    f.flush()?;
    f.sync_all()?;

    Ok(())
}

/// Create a mutation intent for a project lifecycle operation.
pub fn create_project_intent(
    advanced_dir: &Path,
    operation: MutationOperation,
    project_name: &str,
    baseline_snapshot_id: &str,
) -> Result<AdvancedMutationIntent> {
    let transaction_id = format!(
        "{}-{}-{}",
        serde_json::to_string(&operation).unwrap_or_default().trim_matches('"'),
        project_name,
        chrono::Utc::now().format("%Y%m%dT%H%M%SZ")
    );

    let intent = AdvancedMutationIntent {
        transaction_id,
        operation,
        phase: MutationPhase::Prepared,
        baseline_snapshot_id: baseline_snapshot_id.to_string(),
        staged_snapshot_id: None,
        before_project_fingerprint: None,
        after_project_fingerprint: None,
        affected_registration_keys: vec![],
        last_error: None,
        created_at: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        updated_at: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
    };

    create_intent(advanced_dir, &intent)?;
    Ok(intent)
}

/// Complete a project lifecycle intent (mark as completed).
pub fn complete_project_intent(advanced_dir: &Path, intent: AdvancedMutationIntent) -> Result<()> {
    advance_intent(advanced_dir, intent, MutationPhase::Completed)?;
    Ok(())
}

/// Fail a project lifecycle intent with an error.
pub fn fail_project_intent(advanced_dir: &Path, mut intent: AdvancedMutationIntent, error: &str) -> Result<()> {
    intent.last_error = Some(error.to_string());
    create_intent(advanced_dir, &intent)?;
    advance_intent(advanced_dir, intent, MutationPhase::Failed)?;
    Ok(())
}

/// Notify the advanced Source system about a project lifecycle change.
///
/// This is called by project services (new, import, rename, archive, remove)
/// when Lance mode is active. It records a durable intent that can be
/// recovered after a crash. The intent is automatically completed on success.
///
/// Project services should call this BEFORE committing the filesystem change
/// (phase = Prepared), then call `complete_project_intent` after success,
/// or `fail_project_intent` on failure.
pub fn notify_project_change(
    repo_root: &Path,
    operation: MutationOperation,
    project_name: &str,
    baseline_snapshot_id: &str,
) -> Result<AdvancedMutationIntent> {
    let advanced_dir = repo_root.join(".mind").join("source").join("advanced");

    // Only create intents if the advanced directory already exists (Lance is active)
    if !advanced_dir.exists() {
        // Return a no-op intent
        return Ok(AdvancedMutationIntent {
            transaction_id: "noop".to_string(),
            operation,
            phase: MutationPhase::Completed,
            baseline_snapshot_id: baseline_snapshot_id.to_string(),
            staged_snapshot_id: None,
            before_project_fingerprint: None,
            after_project_fingerprint: None,
            affected_registration_keys: vec![],
            last_error: None,
            created_at: String::new(),
            updated_at: String::new(),
        });
    }

    create_project_intent(&advanced_dir, operation, project_name, baseline_snapshot_id)
}

/// Read an existing intent from disk.
pub fn read_intent(advanced_dir: &Path, transaction_id: &str) -> Result<AdvancedMutationIntent> {
    let path = transaction_path(advanced_dir, transaction_id);
    let data = fs::read_to_string(&path)
        .map_err(|e| MfError::advanced_store(format!("cannot read transaction {transaction_id}: {e}"), None))?;
    serde_json::from_str(&data).map_err(MfError::Json)
}

/// Update an intent to a new phase and persist.
pub fn advance_intent(advanced_dir: &Path, mut intent: AdvancedMutationIntent, new_phase: MutationPhase) -> Result<()> {
    intent.phase = new_phase;
    intent.updated_at = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    create_intent(advanced_dir, &intent)
}

/// Delete a completed or discarded transaction record.
pub fn remove_intent(advanced_dir: &Path, transaction_id: &str) -> Result<()> {
    let path = transaction_path(advanced_dir, transaction_id);
    if path.exists() {
        fs::remove_file(&path)?;
    }
    Ok(())
}

/// List all pending (non-completed, non-failed) transaction intents.
pub fn list_pending_intents(advanced_dir: &Path) -> Result<Vec<AdvancedMutationIntent>> {
    let dir = transactions_dir(advanced_dir);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut intents = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "json")
            && let Ok(data) = fs::read_to_string(&path)
            && let Ok(intent) = serde_json::from_str::<AdvancedMutationIntent>(&data)
        {
            match intent.phase {
                MutationPhase::Completed | MutationPhase::Failed => {}
                _ => intents.push(intent),
            }
        }
    }

    intents.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    Ok(intents)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_intent(id: &str) -> AdvancedMutationIntent {
        AdvancedMutationIntent {
            transaction_id: id.into(),
            operation: MutationOperation::Rename,
            phase: MutationPhase::Prepared,
            baseline_snapshot_id: "snap-1".into(),
            staged_snapshot_id: None,
            before_project_fingerprint: None,
            after_project_fingerprint: None,
            affected_registration_keys: vec![],
            last_error: None,
            created_at: "2026-07-13T00:00:00Z".into(),
            updated_at: "2026-07-13T00:00:00Z".into(),
        }
    }

    #[test]
    fn intent_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let intent = test_intent("txn-1");
        create_intent(dir.path(), &intent).unwrap();

        let back = read_intent(dir.path(), "txn-1").unwrap();
        assert_eq!(back.transaction_id, "txn-1");
        assert_eq!(back.operation, MutationOperation::Rename);
        assert_eq!(back.phase, MutationPhase::Prepared);
    }

    #[test]
    fn advance_and_list() {
        let dir = tempfile::tempdir().unwrap();
        let intent = test_intent("txn-2");
        create_intent(dir.path(), &intent).unwrap();

        advance_intent(dir.path(), intent, MutationPhase::FactsCommitted).unwrap();

        let pending = list_pending_intents(dir.path()).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].phase, MutationPhase::FactsCommitted);
    }

    #[test]
    fn completed_intents_not_listed_as_pending() {
        let dir = tempfile::tempdir().unwrap();
        let intent = test_intent("txn-3");
        create_intent(dir.path(), &intent).unwrap();
        advance_intent(dir.path(), intent, MutationPhase::Completed).unwrap();

        let pending = list_pending_intents(dir.path()).unwrap();
        assert!(pending.is_empty());
    }

    #[test]
    fn remove_intent_cleans_up() {
        let dir = tempfile::tempdir().unwrap();
        create_intent(dir.path(), &test_intent("txn-4")).unwrap();
        remove_intent(dir.path(), "txn-4").unwrap();
        assert!(list_pending_intents(dir.path()).unwrap().is_empty());
    }
}
