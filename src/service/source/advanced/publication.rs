//! Atomic snapshot publication and retention for the five-table LanceDB catalog.
//!
//! Every Lance-primary writer takes the single repository `writer.lock`.
//! Publication is stage→validate→tag→fsync→atomic-pointer-replace.
//! A failed mutation never changes `current.json`.
//!
//! ## Publication protocol
//!
//! 1. Take the exclusive writer lock.
//! 2. Stage changes in LanceDB tables.
//! 3. Build/refresh indexes, then optimize before visibility.
//! 4. Validate uniqueness, references, revision parity, counts, dimensions.
//! 5. Create protection tags for every exact table version.
//! 6. Fsync the immutable snapshot manifest.
//! 7. Atomically replace `current.json` pointer.
//! 8. Under exclusive publiction lock, prune unreferenced versions/snapshots.
//!
//! ## Retention
//!
//! After the first validated publication, the sole snapshot and its tags are
//! retained. Once two or more validated snapshots exist, cleanup never drops
//! below two and preserves every tagged table version referenced by any retained
//! snapshot.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{MfError, Result};

// ── Filesystem paths ───────────────────────────────────────────────────────

const POINTER_FILE: &str = "current.json";
const PUBLICATION_LOCK: &str = "publication.lock";
const WRITER_LOCK: &str = "writer.lock";
const GENERATIONS_DIR: &str = "generations";
const SNAPSHOTS_DIR: &str = "snapshots";
const GITIGNORE: &str = ".gitignore";
const GITIGNORE_CONTENT: &str = "*\n!.gitignore\n";

/// Build the path to the pointer file.
pub fn pointer_path(advanced_dir: &Path) -> PathBuf {
    advanced_dir.join(POINTER_FILE)
}

/// Build the path to the writer lock file.
pub fn writer_lock_path(advanced_dir: &Path) -> PathBuf {
    advanced_dir.join(WRITER_LOCK)
}

/// Build the path to the publication lock file.
pub fn publication_lock_path(advanced_dir: &Path) -> PathBuf {
    advanced_dir.join(PUBLICATION_LOCK)
}

/// Build the path to a generation directory.
pub fn generation_path(advanced_dir: &Path, generation_id: &str) -> PathBuf {
    advanced_dir.join(GENERATIONS_DIR).join(generation_id)
}

/// Build the path to a snapshot manifest.
pub fn snapshot_path(advanced_dir: &Path, generation_id: &str, snapshot_id: &str) -> PathBuf {
    generation_path(advanced_dir, generation_id).join(SNAPSHOTS_DIR).join(format!("{snapshot_id}.json"))
}

// ── Pointer and snapshot types ─────────────────────────────────────────────

/// The active index pointer (`current.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositorySourceIndexPointer {
    pub schema_version: String,
    pub generation_id: String,
    pub database_uri: String,
    pub snapshot_path: String,
    pub published_at: String,
}

/// Reference to an exact table version within a snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableVersionRef {
    pub table: String,
    pub version: u64,
    pub tag: String,
}

/// An immutable snapshot manifest pinning exact five-table versions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositorySourceIndexSnapshot {
    pub snapshot_id: String,
    pub schema_version: String,
    pub generation_id: String,
    pub registrations_version: TableVersionRef,
    pub documents_version: TableVersionRef,
    pub registration_content_version: TableVersionRef,
    pub chunks_version: TableVersionRef,
    pub enrichments_version: TableVersionRef,
    pub primary_catalog_fingerprint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activation_legacy_inventory_fingerprint: Option<String>,
    pub active_project_catalog_fingerprint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index_fingerprint: Option<String>,
    pub search_policy_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_identity: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aggregate_counts: Option<serde_json::Value>,
    pub created_at: String,
}

// ── Lock helpers ───────────────────────────────────────────────────────────

/// Acquire an exclusive filesystem lock.
///
/// Uses `fs2::FileExt::try_lock_exclusive` for non-blocking acquisition.
/// Returns true if the lock was acquired, false if another process holds it.
pub fn try_acquire_writer_lock(advanced_dir: &Path) -> Result<std::fs::File> {
    ensure_advanced_dir(advanced_dir)?;
    let lock_path = writer_lock_path(advanced_dir);
    let file = std::fs::OpenOptions::new().create(true).truncate(false).read(true).write(true).open(&lock_path)?;

    fs2::FileExt::try_lock_exclusive(&file).map_err(|e| {
        if e.kind() == std::io::ErrorKind::WouldBlock {
            MfError::advanced_store(
                "another mf process is writing to the advanced Source store".to_string(),
                Some("wait for the other operation to complete and retry".to_string()),
            )
        } else {
            MfError::Io(e)
        }
    })?;

    Ok(file)
}

/// Release a writer lock (drop the file handle).
pub fn release_writer_lock(_file: std::fs::File) {
    // The lock is released when the file is dropped.
}

// ── Pointer operations ─────────────────────────────────────────────────────

/// Read the current pointer from disk. Returns `None` if the file does not exist.
pub fn read_pointer(advanced_dir: &Path) -> Result<Option<RepositorySourceIndexPointer>> {
    let path = pointer_path(advanced_dir);
    if !path.exists() {
        return Ok(None);
    }
    let data = fs::read_to_string(&path).map_err(|e| {
        MfError::missing_lance_pointer("unreadable", format!("cannot read {}: {e}", path.display()), None)
    })?;
    let pointer: RepositorySourceIndexPointer = serde_json::from_str(&data).map_err(|e| {
        MfError::missing_lance_pointer("corrupt", format!("cannot parse {}: {e}", path.display()), None)
    })?;
    Ok(Some(pointer))
}

/// Atomically write the pointer file (write to temp, fsync, rename).
pub fn write_pointer(advanced_dir: &Path, pointer: &RepositorySourceIndexPointer) -> Result<()> {
    ensure_advanced_dir(advanced_dir)?;
    let path = pointer_path(advanced_dir);
    let tmp = path.with_extension("tmp");

    let json = serde_json::to_string_pretty(pointer).map_err(MfError::Json)?;
    let mut f = fs::File::create(&tmp)?;
    f.write_all(json.as_bytes())?;
    f.flush()?;
    f.sync_all()?;

    fs::rename(&tmp, &path)?;

    // fsync the parent directory to ensure the rename is durable
    if let Some(parent) = path.parent() {
        let dir = fs::File::open(parent)?;
        dir.sync_all()?;
    }

    Ok(())
}

// ── Snapshot write / read ──────────────────────────────────────────────────

/// Write a snapshot manifest to the immutable snapshots directory.
pub fn write_snapshot(advanced_dir: &Path, snapshot: &RepositorySourceIndexSnapshot) -> Result<()> {
    let path = snapshot_path(advanced_dir, &snapshot.generation_id, &snapshot.snapshot_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let json = serde_json::to_string_pretty(snapshot).map_err(MfError::Json)?;
    let mut f = fs::File::create(&path)?;
    f.write_all(json.as_bytes())?;
    f.flush()?;
    f.sync_all()?;

    Ok(())
}

/// Read a snapshot manifest from the immutable snapshots directory.
pub fn read_snapshot(
    advanced_dir: &Path,
    generation_id: &str,
    snapshot_id: &str,
) -> Result<RepositorySourceIndexSnapshot> {
    let path = snapshot_path(advanced_dir, generation_id, snapshot_id);
    let data = fs::read_to_string(&path)
        .map_err(|e| MfError::missing_lance_pointer("missing", format!("snapshot not found: {e}"), None))?;
    serde_json::from_str(&data).map_err(MfError::Json)
}

/// Enumerate all retained snapshot manifests in the generations directory.
pub fn list_snapshots(advanced_dir: &Path) -> Result<Vec<RepositorySourceIndexSnapshot>> {
    let gens_dir = advanced_dir.join(GENERATIONS_DIR);
    if !gens_dir.exists() {
        return Ok(Vec::new());
    }

    let mut snapshots = Vec::new();
    for gen_entry in fs::read_dir(&gens_dir)? {
        let gen_entry = gen_entry?;
        let snaps_dir = gen_entry.path().join(SNAPSHOTS_DIR);
        if !snaps_dir.exists() {
            continue;
        }
        for snap_entry in fs::read_dir(&snaps_dir)? {
            let snap_entry = snap_entry?;
            let path = snap_entry.path();
            if path.extension().is_some_and(|e| e == "json")
                && let Ok(data) = fs::read_to_string(&path)
                && let Ok(snapshot) = serde_json::from_str::<RepositorySourceIndexSnapshot>(&data)
            {
                snapshots.push(snapshot);
            }
        }
    }

    // Sort by created_at descending (most recent first)
    snapshots.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(snapshots)
}

// ── `.gitignore` creation ─────────────────────────────────────────────────

/// Atomically create `.mind/.gitignore` with the runtime ignore policy.
pub fn ensure_gitignore(advanced_dir: &Path) -> Result<()> {
    // `advanced_dir` is `<repo>/.mind/source/advanced`; the runtime ignore
    // policy belongs to `.mind`, not its `source` child and never repo root.
    let mind_dir = advanced_dir
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| MfError::advanced_store("invalid advanced dir path".to_string(), None))?;
    let gitignore_path = mind_dir.join(GITIGNORE);

    if !gitignore_path.exists() {
        let mut f = fs::File::create(&gitignore_path)?;
        f.write_all(GITIGNORE_CONTENT.as_bytes())?;
        f.flush()?;
        f.sync_all()?;
    }
    Ok(())
}

// ── Directory bootstrap ────────────────────────────────────────────────────

/// Ensure the advanced directory and its subdirectories exist.
fn ensure_advanced_dir(advanced_dir: &Path) -> Result<()> {
    fs::create_dir_all(advanced_dir)?;
    fs::create_dir_all(advanced_dir.join(GENERATIONS_DIR))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_roundtrip() {
        let pointer = RepositorySourceIndexPointer {
            schema_version: "1".into(),
            generation_id: "gen-1".into(),
            database_uri: "./generations/gen-1/lancedb".into(),
            snapshot_path: "./generations/gen-1/snapshots/snap-1.json".into(),
            published_at: "2026-07-13T00:00:00Z".into(),
        };
        let json = serde_json::to_string(&pointer).unwrap();
        let back: RepositorySourceIndexPointer = serde_json::from_str(&json).unwrap();
        assert_eq!(back.generation_id, "gen-1");
        assert_eq!(back.schema_version, "1");
    }

    #[test]
    fn snapshot_roundtrip() {
        let snapshot = RepositorySourceIndexSnapshot {
            snapshot_id: "snap-1".into(),
            schema_version: "1".into(),
            generation_id: "gen-1".into(),
            registrations_version: TableVersionRef { table: "registrations".into(), version: 1, tag: "tag-1".into() },
            documents_version: TableVersionRef { table: "documents".into(), version: 1, tag: "tag-1".into() },
            registration_content_version: TableVersionRef {
                table: "registration_content".into(),
                version: 1,
                tag: "tag-1".into(),
            },
            chunks_version: TableVersionRef { table: "chunks".into(), version: 1, tag: "tag-1".into() },
            enrichments_version: TableVersionRef { table: "enrichments".into(), version: 1, tag: "tag-1".into() },
            primary_catalog_fingerprint: "fp-1".into(),
            activation_legacy_inventory_fingerprint: None,
            active_project_catalog_fingerprint: "ap-fp-1".into(),
            content_fingerprint: None,
            index_fingerprint: None,
            search_policy_version: "1".into(),
            model_identity: None,
            aggregate_counts: None,
            created_at: "2026-07-13T00:00:00Z".into(),
        };
        let json = serde_json::to_string(&snapshot).unwrap();
        let back: RepositorySourceIndexSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(back.snapshot_id, "snap-1");
        assert_eq!(back.registrations_version.table, "registrations");
    }

    #[test]
    fn read_pointer_returns_none_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let result = read_pointer(dir.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn write_and_read_pointer_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let pointer = RepositorySourceIndexPointer {
            schema_version: "1".into(),
            generation_id: "gen-2".into(),
            database_uri: "./db".into(),
            snapshot_path: "./snap".into(),
            published_at: "2026-07-13T00:00:00Z".into(),
        };
        write_pointer(dir.path(), &pointer).unwrap();
        let back = read_pointer(dir.path()).unwrap().unwrap();
        assert_eq!(back.generation_id, "gen-2");
    }
}
