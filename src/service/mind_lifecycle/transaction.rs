//! A recoverable transaction on one local filesystem. Backups precede all
//! authored writes; recovery derives progress from disk, never a step counter.
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Component, Path};

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{MfError, Result};

const PREPARING: &str = ".mind-forge/lifecycle-preparing";
const ABORTED: &str = ".mind-forge/lifecycle-preparation-aborted";
const STATE: &str = ".mind-forge/lifecycle";
const JOURNAL: &str = ".mind-forge/lifecycle/journal.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Entry {
    File { bytes: Vec<u8>, mode: u32 },
    Directory { mode: u32 },
}

impl Entry {
    pub fn hash(&self) -> String {
        digest(&serde_json::to_vec(self).expect("entry is serializable"))
    }

    fn is_dir(&self) -> bool {
        matches!(self, Self::Directory { .. })
    }
}

pub type Snapshot = BTreeMap<String, Entry>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub version: u32,
    pub root: String,
    pub operation: String,
    pub watches: Vec<String>,
    pub before: Snapshot,
    pub after: Snapshot,
    pub digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WritePreview {
    pub path: String,
    pub before_hash: Option<String>,
    pub after_hash: Option<String>,
    pub action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Prepared,
    Complete,
    RolledBack,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Journal {
    pub plan: Plan,
    pub phase: Phase,
}

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn rejected(kind: &'static str, reason: impl std::fmt::Display) -> MfError {
    MfError::Lifecycle { code: kind, message: reason.to_string() }
}

fn relative(path: &str) -> Result<()> {
    if path.is_empty()
        || Path::new(path).components().any(|c| !matches!(c, Component::Normal(_)))
        || path.split('/').any(|p| p == ".git" || p == ".mind-forge")
    {
        return Err(rejected("unsafe_path", path));
    }
    Ok(())
}

fn safe_path(root: &Path, path: &str) -> Result<std::path::PathBuf> {
    relative(path)?;
    let mut result = root.to_path_buf();
    for component in Path::new(path).components() {
        result.push(component);
        if result.exists() {
            same_device(root, &result)?;
        }
        if fs::symlink_metadata(&result).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(rejected("unsafe_path", format!("symlink {}", result.display())));
        }
    }
    Ok(result)
}

#[cfg(unix)]
fn mode(metadata: &fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o7777
}

#[cfg(not(unix))]
fn mode(_: &fs::Metadata) -> u32 {
    0
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_mode(_: &Path, _: u32) -> Result<()> {
    Err(rejected("unsupported_filesystem", "Unix required"))
}

fn same_device(root: &Path, path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if fs::metadata(root)?.dev() != fs::metadata(path)?.dev() {
            return Err(rejected("cross_device", path.display()));
        }
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = (root, path);
        Err(rejected("unsupported_filesystem", "Unix required"))
    }
}

/// Only explicitly known local filesystems are accepted. Advisory locks cannot
/// establish exclusivity for NFS/SMB or a shared remote store.
fn local_filesystem(root: &Path) -> Result<()> {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        use std::os::unix::ffi::OsStrExt;
        let name = std::ffi::CString::new(root.as_os_str().as_bytes()).map_err(|_| rejected("unsafe_path", "NUL"))?;
        let mut info = std::mem::MaybeUninit::<libc::statfs>::uninit();
        // SAFETY: name is NUL-terminated and info points to writable statfs storage.
        if unsafe { libc::statfs(name.as_ptr(), info.as_mut_ptr()) } != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        // SAFETY: statfs returned success and initialized info.
        let info = unsafe { info.assume_init() };
        #[cfg(target_os = "macos")]
        let supported = {
            // SAFETY: f_fstypename is the OS-provided NUL-terminated filesystem name.
            let name = unsafe { std::ffi::CStr::from_ptr(info.f_fstypename.as_ptr()) }.to_bytes();
            matches!(name, b"apfs" | b"hfs")
        };
        #[cfg(target_os = "linux")]
        let supported = matches!(info.f_type as u64, 0xef53 | 0x9123683e | 0x58465342 | 0x01021994);
        if !supported {
            return Err(rejected(
                "unsupported_filesystem",
                "only known local APFS/HFS/ext/XFS/Btrfs/tmpfs filesystems are supported",
            ));
        }
        Ok(())
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = root;
        Err(rejected("unsupported_filesystem", "platform has no verified local-filesystem implementation"))
    }
}

/// No directories, cache, locks, or journals are created by inspection.
pub fn snapshot(root: &Path, watches: &[String]) -> Result<Snapshot> {
    let mut result = Snapshot::new();
    for watch in watches {
        let path = if watch == "." { root.to_path_buf() } else { safe_path(root, watch)? };
        if !path.exists() {
            continue;
        }
        for entry in walkdir::WalkDir::new(&path)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| e.depth() != 1 || !matches!(e.file_name().to_str(), Some(".git" | ".mind-forge")))
        {
            let entry = entry.map_err(|e| rejected("inventory_failed", e))?;
            let rel = entry
                .path()
                .strip_prefix(root)
                .map_err(|e| rejected("unsafe_path", e))?
                .to_str()
                .ok_or_else(|| rejected("unsafe_path", "non UTF-8 path"))?
                .to_owned();
            if rel.is_empty() {
                continue;
            }
            safe_path(root, &rel)?;
            let metadata = fs::symlink_metadata(entry.path())?;
            same_device(root, entry.path())?;
            let value = if metadata.is_file() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    if metadata.nlink() != 1 {
                        return Err(rejected("unsafe_path", format!("hardlinked file {rel}")));
                    }
                }
                Entry::File { bytes: fs::read(entry.path())?, mode: mode(&metadata) }
            } else if metadata.is_dir() {
                Entry::Directory { mode: mode(&metadata) }
            } else {
                return Err(rejected("unsafe_path", format!("non-regular entry {rel}")));
            };
            result.insert(rel, value);
        }
    }
    Ok(result)
}

impl Plan {
    pub fn new(
        root: &Path,
        operation: String,
        watches: Vec<String>,
        before: Snapshot,
        after: Snapshot,
    ) -> Result<Self> {
        let root = fs::canonicalize(root)?;
        local_filesystem(&root)?;

        let mut plan = Self {
            version: 1,
            root: root.to_string_lossy().into_owned(),
            operation,
            watches,
            before,
            after,
            digest: String::new(),
        };
        plan.validate_shape()?;
        plan.digest = plan.compute_digest()?;
        Ok(plan)
    }

    fn compute_digest(&self) -> Result<String> {
        Ok(digest(&serde_json::to_vec(&(
            self.version,
            &self.root,
            &self.operation,
            &self.watches,
            &self.before,
            &self.after,
        ))?))
    }

    fn validate_shape(&self) -> Result<()> {
        if self.version != 1 {
            return Err(rejected("invalid_plan", "unsupported version"));
        }
        for watch in &self.watches {
            if watch != "." {
                relative(watch)?;
            }
        }
        for state in [&self.before, &self.after] {
            let mut folded = BTreeSet::new();
            for (path, entry) in state {
                relative(path)?;
                if let Entry::Directory { mode } = entry
                    && mode & 0o700 != 0o700
                {
                    return Err(rejected(
                        "unsupported_permissions",
                        format!("directory requires owner rwx for recoverable traversal: {path}"),
                    ));
                }
                if !self.watches.iter().any(|w| w == "." || Path::new(path).starts_with(w)) {
                    return Err(rejected("invalid_plan", "write escapes watched inventory"));
                }
                if !folded.insert(path.to_lowercase()) {
                    return Err(rejected("path_conflict", path));
                }
                if let Some(parent) = Path::new(path).parent().filter(|p| !p.as_os_str().is_empty()) {
                    if let Some(parent_entry) = state.get(&parent.to_string_lossy().into_owned()) {
                        if !parent_entry.is_dir() {
                            return Err(rejected("path_conflict", path));
                        }
                    } else if self.watches.iter().any(|w| w == "." || parent.starts_with(w)) {
                        return Err(rejected("invalid_plan", format!("missing parent for {path}")));
                    }
                }
                if let Some(before) = self.before.get(path)
                    && before.is_dir() != entry.is_dir()
                {
                    return Err(rejected("path_conflict", "file/directory replacement is not supported"));
                }
            }
        }
        Ok(())
    }

    pub fn preview(&self) -> Vec<WritePreview> {
        self.before
            .keys()
            .chain(self.after.keys())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter(|p| self.before.get(*p) != self.after.get(*p))
            .map(|path| WritePreview {
                path: path.clone(),
                before_hash: self.before.get(path).map(Entry::hash),
                after_hash: self.after.get(path).map(Entry::hash),
                action: if !self.after.contains_key(path) {
                    "remove"
                } else if !self.before.contains_key(path) {
                    "create"
                } else {
                    "update"
                }
                .into(),
            })
            .collect()
    }
}

fn state_path(root: &Path) -> Result<std::path::PathBuf> {
    let namespace = root.join(".mind-forge");
    for path in [
        &namespace,
        &root.join(STATE),
        &root.join(PREPARING),
        &root.join(PREPARING).join("journal.json"),
        &root.join(PREPARING).join("journal.json.next"),
        &root.join(ABORTED),
        &root.join(JOURNAL),
        &root.join(format!("{JOURNAL}.next")),
        &namespace.join("write.lock"),
        &root.join(STATE).join("payload"),
        &root.join(STATE).join("directory"),
    ] {
        if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(rejected("unsafe_path", path.display()));
        }
        if path.exists() {
            same_device(root, path)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                let metadata = fs::symlink_metadata(path)?;
                if !metadata.is_file() && !metadata.is_dir() {
                    return Err(rejected(
                        "unsafe_path",
                        format!("non-regular transaction metadata: {}", path.display()),
                    ));
                }
                if metadata.is_file() && metadata.nlink() != 1 {
                    return Err(rejected(
                        "unsafe_path",
                        format!("hardlinked transaction metadata: {}", path.display()),
                    ));
                }
            }
        }
    }
    Ok(root.join(STATE))
}

/// Caller must hold this lock across validation, journal preparation, and apply.
/// Other mf mutation families must use this same lock when the CLI is integrated.
pub struct WriteLock(File);

impl Drop for WriteLock {
    fn drop(&mut self) {
        // Explicit unlock also releases a lock inherited by a concurrently
        // forked child before its CLOEXEC descriptors have been closed.
        let _ = FileExt::unlock(&self.0);
    }
}

pub fn write_lock(root: &Path) -> Result<WriteLock> {
    local_filesystem(root)?;
    state_path(root)?;
    // flock on the root directory coordinates every writer without creating a
    // lock file on rejected commands or previews. The root inode is stable for
    // all operations supported by this worktree-local protocol.
    let lock = File::open(root)?;
    lock.try_lock_exclusive().map_err(|e| rejected("write_locked", e))?;
    Ok(WriteLock(lock))
}

fn sync_dir(path: &Path) -> Result<()> {
    File::open(path)?.sync_all()?;
    Ok(())
}

/// Persist the backup and desired content in one fsynced journal. The .next
/// image is itself sufficient for recovery if interrupted before its rename.
fn save_journal(root: &Path, journal: &Journal) -> Result<()> {
    save_journal_at(&root.join(STATE), journal)
}

fn save_journal_at(directory: &Path, journal: &Journal) -> Result<()> {
    let path = directory.join("journal.json");
    let next = directory.join("journal.json.next");
    let mut file = OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&next)?;
    set_mode(&next, 0o600)?;
    file.write_all(&serde_json::to_vec(journal)?)?;
    file.sync_all()?;
    fs::rename(next, &path)?;
    sync_dir(path.parent().expect("journal parent"))
}

pub fn read_journal(root: &Path) -> Result<Option<Journal>> {
    state_path(root)?;
    if !root.join(STATE).exists() && root.join(PREPARING).exists() {
        return Err(rejected(
            "recovery_required",
            "unpublished preparation; use lifecycle resume or rollback (no external plan required)",
        ));
    }
    let path = if root.join(JOURNAL).is_file() { root.join(JOURNAL) } else { root.join(format!("{JOURNAL}.next")) };
    if !path.exists() {
        if root.join(STATE).exists() {
            return Err(rejected(
                "recovery_required",
                "journal preparation interrupted before a durable journal; authored writes have not begun",
            ));
        }
        return Ok(None);
    }
    let journal: Journal = serde_json::from_slice(&fs::read(path)?).map_err(|e| {
        rejected(
            "invalid_journal",
            format!("incomplete or invalid journal; retain evidence and recover with original plan: {e}"),
        )
    })?;
    journal.plan.validate_shape()?;
    if journal.plan.compute_digest()? != journal.plan.digest {
        return Err(rejected("invalid_journal", "digest mismatch"));
    }
    Ok(Some(journal))
}

pub fn check_recovery(root: &Path) -> Result<()> {
    if let Some(journal) = read_journal(root)?
        && journal.phase == Phase::Prepared
    {
        return Err(rejected("recovery_required", &journal.plan.digest));
    }
    Ok(())
}

fn verify_root(root: &Path, plan: &Plan) -> Result<()> {
    if fs::canonicalize(root)?.to_string_lossy() != plan.root {
        return Err(rejected("wrong_worktree", "plan is bound to a different root"));
    }
    local_filesystem(root)?;

    plan.validate_shape()?;
    if plan.compute_digest()? != plan.digest {
        return Err(rejected("invalid_plan", "digest mismatch"));
    }
    Ok(())
}

/// Read-only validation for saved plan previews.
pub fn validate(root: &Path, plan: &Plan) -> Result<()> {
    verify_root(root, plan)?;
    check_recovery(root)?;
    if snapshot(root, &plan.watches)? != plan.before {
        return Err(rejected("stale_plan", "watched inventory changed"));
    }
    Ok(())
}

fn verify_partial(root: &Path, plan: &Plan) -> Result<()> {
    let current = snapshot(root, &plan.watches)?;
    let paths: BTreeSet<_> = current.keys().chain(plan.before.keys()).chain(plan.after.keys()).collect();
    for path in paths {
        let actual = current.get(path);
        if actual != plan.before.get(path) && actual != plan.after.get(path) {
            return Err(rejected("recovery_conflict", format!("later edit or unknown entry: {path}")));
        }
    }
    Ok(())
}

fn inject(step: usize, fail_after: Option<usize>) -> Result<()> {
    if fail_after == Some(step) { Err(rejected("injected_failure", format!("durable stage {step}"))) } else { Ok(()) }
}

pub fn apply(root: &Path, plan: &Plan, fail_after: Option<usize>) -> Result<Journal> {
    verify_root(root, plan)?;
    let _lock = write_lock(root)?;
    if let Some(journal) = read_journal(root)? {
        if journal.plan.digest == plan.digest && journal.phase == Phase::Complete {
            return Ok(journal);
        }
        if journal.phase == Phase::Prepared {
            return Err(rejected("recovery_required", journal.plan.digest));
        }
    }
    if snapshot(root, &plan.watches)? != plan.before {
        return Err(rejected("stale_plan", "watched inventory changed"));
    }
    let state = state_path(root)?;
    fs::create_dir_all(root.join(".mind-forge"))?;
    sync_dir(root)?;
    let journal = Journal { plan: plan.clone(), phase: Phase::Prepared };
    if !state.exists() {
        // Publish the entire prepared directory atomically. Before publication,
        // only tool-owned preparation files can have changed, never authored files.
        preparation_inject("namespace")?;
        let preparing = root.join(PREPARING);
        fs::DirBuilder::new().mode(0o700).create(&preparing)?;
        sync_dir(preparing.parent().unwrap())?;
        preparation_inject("directory")?;
        if preparation_failure("partial") {
            let bytes = serde_json::to_vec(&journal)?;
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(preparing.join("journal.json.next"))?;
            file.write_all(&bytes[..bytes.len() / 2])?;
            file.sync_all()?;
            preparation_inject("partial")?;
        }
        save_journal_at(&preparing, &journal)?;
        preparation_inject("backup")?;
        fs::rename(&preparing, &state)?;
        sync_dir(state.parent().unwrap())?;
        preparation_inject("publish")?;
    } else {
        set_mode(&state, 0o700)?;
        save_journal(root, &journal)?;
    }
    inject(0, fail_after)?;
    finish(root, journal, false, fail_after)
}

// Debug builds expose named preparation boundaries solely for CLI fault tests.
fn preparation_failure(stage: &str) -> bool {
    cfg!(debug_assertions) && std::env::var("MF_TEST_LIFECYCLE_PREPARE_FAILURE").is_ok_and(|value| value == stage)
}
fn preparation_inject(stage: &str) -> Result<()> {
    if preparation_failure(stage) { Err(rejected("injected_failure", format!("preparation {stage}"))) } else { Ok(()) }
}

/// Resolve an unpublished initial preparation without a user-saved plan.
/// None means normal journal recovery should continue. true means preparation
/// was archived without authored changes; rerun the original command to retry.
pub fn recover_unpublished(root: &Path, rollback: bool) -> Result<bool> {
    let _lock = write_lock(root)?;
    if root.join(STATE).exists() {
        return Ok(false);
    }
    let preparing = root.join(PREPARING);
    if !preparing.exists() {
        return Ok(root.join(ABORTED).exists());
    }
    for entry in fs::read_dir(&preparing)? {
        let entry = entry?;
        if !matches!(entry.file_name().to_str(), Some("journal.json" | "journal.json.next")) {
            return Err(rejected("invalid_journal", "unknown preparation entry; evidence preserved"));
        }
    }
    let path = if preparing.join("journal.json").exists() {
        preparing.join("journal.json")
    } else {
        preparing.join("journal.json.next")
    };
    let journal = if path.exists() { serde_json::from_slice::<Journal>(&fs::read(&path)?).ok() } else { None };
    if let Some(journal) = &journal {
        verify_root(root, &journal.plan)?;
        if journal.phase != Phase::Prepared {
            return Err(rejected("invalid_journal", "unpublished journal must be prepared"));
        }
        if !rollback {
            // The complete backup is sufficient even when its final .next
            // rename was interrupted. Finish that rename before publication.
            if path.file_name().is_some_and(|n| n == "journal.json.next") {
                fs::rename(&path, preparing.join("journal.json"))?;
                sync_dir(&preparing)?;
            }
            fs::rename(&preparing, root.join(STATE))?;
            sync_dir(preparing.parent().unwrap())?;
            return Ok(false);
        }
    }
    // Incomplete preparation has no reconstructable intent, but the protocol
    // proves that it has not begun authored writes. Archive rather than delete.
    // Publish an abort receipt first so a crash after archival remains idempotent.
    let receipt = root.join(ABORTED);
    if !receipt.exists() {
        let file = OpenOptions::new().write(true).create_new(true).mode(0o600).open(&receipt)?;
        file.sync_all()?;
        sync_dir(receipt.parent().unwrap())?;
    }
    let mut serial = 0u64;
    let archive = loop {
        let path = root.join(format!(".mind-forge/lifecycle-preparation-evidence-{serial}"));
        if !path.exists() {
            break path;
        }
        serial += 1;
    };
    fs::rename(&preparing, archive)?;
    sync_dir(preparing.parent().unwrap())?;
    Ok(true)
}

/// Recover an interrupted *initial* backup using the user's original plan.
/// A complete journal always takes precedence. No authored path may have changed.
/// Incomplete preparation evidence is retained before replacing the journal.
pub fn recover_preparation(root: &Path, plan: &Plan, rollback: bool) -> Result<Journal> {
    verify_root(root, plan)?;
    let _lock = write_lock(root)?;
    let state = state_path(root)?;
    if root.join(JOURNAL).exists() || !state.is_dir() {
        return Err(rejected(
            "invalid_journal",
            "initial preparation recovery requires an interrupted initial journal",
        ));
    }
    if let Ok(Some(_)) = read_journal(root) {
        return Err(rejected("recovery_required", "complete backup exists; use resume or rollback"));
    }
    for entry in fs::read_dir(&state)? {
        let entry = entry?;
        if entry.file_name() != "journal.json.next" {
            return Err(rejected("invalid_journal", "unknown preparation evidence; refusing to replace it"));
        }
    }
    if snapshot(root, &plan.watches)? != plan.before {
        return Err(rejected("recovery_conflict", "initial recovery requires unchanged complete before inventory"));
    }
    let next = root.join(format!("{JOURNAL}.next"));
    if next.exists() {
        let evidence =
            root.join(".mind-forge").join(format!("lifecycle-preparation-{}.json", digest(&fs::read(&next)?)));
        if evidence.exists() {
            return Err(rejected("path_conflict", evidence.display()));
        }
        fs::rename(&next, evidence)?;
        sync_dir(&root.join(".mind-forge"))?;
    }
    let journal = Journal { plan: plan.clone(), phase: Phase::Prepared };
    set_mode(&state, 0o700)?;
    save_journal(root, &journal)?;
    finish(root, journal, rollback, None)
}

pub fn recover(root: &Path, rollback: bool, fail_after: Option<usize>) -> Result<Journal> {
    let _lock = write_lock(root)?;
    let journal = read_journal(root)?.ok_or_else(|| rejected("journal_missing", "no transaction to recover"))?;
    verify_root(root, &journal.plan)?;
    let desired = if rollback { Phase::RolledBack } else { Phase::Complete };
    if journal.phase == desired {
        return Ok(journal);
    }
    if journal.phase != Phase::Prepared {
        return Err(rejected("transaction_finished", "completed transactions cannot change direction"));
    }
    verify_partial(root, &journal.plan)?;
    finish(root, journal, rollback, fail_after)
}

fn finish(root: &Path, mut journal: Journal, rollback: bool, fail_after: Option<usize>) -> Result<Journal> {
    let target = if rollback { &journal.plan.before } else { &journal.plan.after };
    verify_partial(root, &journal.plan)?;
    let current = snapshot(root, &journal.plan.watches)?;
    let mut paths: Vec<_> = current.keys().chain(target.keys()).cloned().collect::<BTreeSet<_>>().into_iter().collect();
    // Create parents first; delete descendants first. Files are copied from the
    // durable journal before any source path is removed.
    paths.sort_by_key(|p| {
        let depth = p.matches('/').count();
        match target.get(p) {
            Some(Entry::Directory { .. }) => (0, depth, p.clone()),
            Some(_) => (1, depth, p.clone()),
            None => (2, usize::MAX - depth, p.clone()),
        }
    });
    let mut step = 0;
    for path in paths {
        if current.get(&path) == target.get(&path) {
            continue;
        }
        verify_partial(root, &journal.plan)?;
        let destination = safe_path(root, &path)?;
        match target.get(&path) {
            Some(Entry::Directory { mode }) => {
                if !destination.exists() {
                    // The directory is never visible with an intermediate umask/mode.
                    let staging = root.join(STATE).join("directory");
                    if staging.exists() {
                        let meta = fs::symlink_metadata(&staging)?;
                        if !meta.is_dir() || meta.file_type().is_symlink() || fs::read_dir(&staging)?.next().is_some() {
                            return Err(rejected("recovery_conflict", "unknown directory staging content"));
                        }
                        fs::remove_dir(&staging)?;
                    }
                    fs::DirBuilder::new().mode(0o700).create(&staging)?;
                    set_mode(&staging, *mode)?;
                    sync_dir(&staging)?;
                    fs::rename(staging, &destination)?;
                } else {
                    set_mode(&destination, *mode)?;
                }
                sync_dir(&destination)?;
            }
            Some(Entry::File { bytes, mode }) => {
                let staging = root.join(STATE).join("payload");
                if fs::symlink_metadata(&staging).is_ok_and(|m| m.file_type().is_symlink()) {
                    return Err(rejected("unsafe_path", staging.display()));
                }
                let mut file = OpenOptions::new().create(true).truncate(true).write(true).open(&staging)?;
                file.write_all(bytes)?;
                set_mode(&staging, *mode)?;
                file.sync_all()?;
                fs::rename(&staging, &destination)?;
            }
            None => {
                if current.get(&path).is_some_and(Entry::is_dir) {
                    fs::remove_dir(&destination)?;
                } else {
                    fs::remove_file(&destination)?;
                }
            }
        }
        sync_dir(destination.parent().expect("checked relative path parent"))?;
        step += 1;
        inject(step, fail_after)?;
    }
    if snapshot(root, &journal.plan.watches)? != *target {
        return Err(rejected("recovery_conflict", "final inventory differs"));
    }
    journal.phase = if rollback { Phase::RolledBack } else { Phase::Complete };
    save_journal(root, &journal)?;
    inject(step + 1, fail_after)?;
    Ok(journal)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, Plan) {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("project/old")).unwrap();
        fs::write(dir.path().join("project/old/block.md"), "# Title\n\nBody\n").unwrap();
        fs::write(dir.path().join("project/old/image.bin"), [0, 255, 1, 128]).unwrap();
        fs::write(dir.path().join("project/mind.yaml"), "content_layout: category\n").unwrap();
        let watches = vec!["project".into()];
        let before = snapshot(dir.path(), &watches).unwrap();
        let mut after: Snapshot =
            before.iter().map(|(path, value)| (path.replace("project/old", "project/new"), value.clone())).collect();
        if let Some(Entry::File { bytes, .. }) = after.get_mut("project/mind.yaml") {
            *bytes = b"content_layout: bundle\n".to_vec();
        }
        let plan = Plan::new(dir.path(), "test move".into(), watches, before, after).unwrap();
        (dir, plan)
    }

    #[test]
    fn incomplete_initial_preparation_requires_original_plan_and_preserves_evidence() {
        for rollback in [false, true] {
            for partial in [false, true] {
                let (dir, plan) = fixture();
                fs::create_dir_all(dir.path().join(STATE)).unwrap();
                if partial {
                    fs::write(dir.path().join(format!("{JOURNAL}.next")), b"{\"plan\":").unwrap();
                }
                assert!(check_recovery(dir.path()).is_err());
                let result = recover_preparation(dir.path(), &plan, rollback).unwrap();
                assert_eq!(result.phase, if rollback { Phase::RolledBack } else { Phase::Complete });
                assert_eq!(
                    snapshot(dir.path(), &plan.watches).unwrap(),
                    if rollback { plan.before } else { plan.after }
                );
                if partial {
                    assert!(
                        fs::read_dir(dir.path().join(".mind-forge")).unwrap().any(|e| e
                            .unwrap()
                            .file_name()
                            .to_string_lossy()
                            .starts_with("lifecycle-preparation-"))
                    );
                }
            }
        }
    }

    #[test]
    fn preparation_recovery_rejects_later_edits_and_unknown_evidence() {
        let (dir, plan) = fixture();
        fs::create_dir_all(dir.path().join(STATE)).unwrap();
        fs::write(dir.path().join("project/old/block.md"), "later edit").unwrap();
        assert_eq!(recover_preparation(dir.path(), &plan, true).unwrap_err().kind(), "recovery_conflict");
        let (dir, plan) = fixture();
        fs::create_dir_all(dir.path().join(STATE)).unwrap();
        fs::write(dir.path().join(STATE).join("payload"), "unknown").unwrap();
        assert_eq!(recover_preparation(dir.path(), &plan, true).unwrap_err().kind(), "invalid_journal");
    }

    #[test]
    fn staged_directory_permission_interruptions_are_recoverable() {
        for rollback in [false, true] {
            let (dir, plan) = fixture();
            apply(dir.path(), &plan, Some(0)).unwrap_err();
            let staging = dir.path().join(STATE).join("directory");
            fs::create_dir(&staging).unwrap();
            set_mode(&staging, 0o711).unwrap();
            recover(dir.path(), rollback, None).unwrap();
            assert_eq!(snapshot(dir.path(), &plan.watches).unwrap(), if rollback { plan.before } else { plan.after });
        }
        let (dir, plan) = fixture();
        let mut after = plan.after.clone();
        after.insert("project/new".into(), Entry::Directory { mode: 0o500 });
        assert_eq!(
            Plan::new(dir.path(), "permissions".into(), plan.watches, plan.before, after).unwrap_err().kind(),
            "unsupported_permissions"
        );
    }

    #[test]
    fn previews_are_zero_write_and_bind_complete_inventory() {
        let (dir, plan) = fixture();
        let before = snapshot(dir.path(), &plan.watches).unwrap();
        let previews = plan.preview();
        assert!(
            previews.iter().any(|p| p.path == "project/mind.yaml" && p.before_hash.is_some() && p.after_hash.is_some())
        );
        assert!(!dir.path().join(".mind-forge").exists());
        assert_eq!(snapshot(dir.path(), &plan.watches).unwrap(), before);
        fs::write(dir.path().join("project/new-review.md"), "late inbound reference").unwrap();
        assert_eq!(apply(dir.path(), &plan, None).unwrap_err().kind(), "stale_plan");
        assert!(!dir.path().join(JOURNAL).exists());
        assert!(dir.path().join("project/old/image.bin").is_file());
    }

    #[test]
    fn every_durable_apply_stage_resumes_and_retries_without_rewriting() {
        let (_, sample) = fixture();
        for stage in 0..=sample.preview().len() + 1 {
            let (dir, plan) = fixture();
            let error = apply(dir.path(), &plan, Some(stage)).unwrap_err();
            assert_eq!(error.kind(), "injected_failure", "stage {stage}: {error}");
            let journal = recover(dir.path(), false, None).unwrap();
            assert_eq!(journal.phase, Phase::Complete);
            assert_eq!(snapshot(dir.path(), &plan.watches).unwrap(), plan.after);
            let saved = fs::read(dir.path().join(JOURNAL)).unwrap();
            recover(dir.path(), false, None).unwrap();
            apply(dir.path(), &plan, None).unwrap();
            assert_eq!(fs::read(dir.path().join(JOURNAL)).unwrap(), saved);
        }
    }

    #[test]
    fn every_unfinished_apply_stage_rolls_back_binary_content_and_permissions() {
        let (_, sample) = fixture();
        for stage in 0..=sample.preview().len() {
            let (dir, plan) = fixture();
            apply(dir.path(), &plan, Some(stage)).unwrap_err();
            assert_eq!(check_recovery(dir.path()).unwrap_err().kind(), "recovery_required");
            let journal = recover(dir.path(), true, None).unwrap();
            assert_eq!(journal.phase, Phase::RolledBack);
            assert_eq!(snapshot(dir.path(), &plan.watches).unwrap(), plan.before);
            let saved = fs::read(dir.path().join(JOURNAL)).unwrap();
            recover(dir.path(), true, None).unwrap();
            assert_eq!(fs::read(dir.path().join(JOURNAL)).unwrap(), saved);
        }
    }

    #[test]
    fn rollback_itself_can_be_interrupted_and_recovered() {
        let (_, sample) = fixture();
        for stage in 0..sample.preview().len() {
            let (dir, plan) = fixture();
            apply(dir.path(), &plan, Some(plan.preview().len())).unwrap_err();
            // Stage zero is the already-durable backup; rollback reuses it.
            let failure = recover(dir.path(), true, Some(stage + 1));
            assert!(failure.is_err());
            recover(dir.path(), true, None).unwrap();
            assert_eq!(snapshot(dir.path(), &plan.watches).unwrap(), plan.before);
        }
    }

    #[test]
    fn later_edits_and_new_entries_are_never_overwritten_by_recovery() {
        for rollback in [true, false] {
            let (dir, plan) = fixture();
            apply(dir.path(), &plan, Some(0)).unwrap_err();
            fs::write(dir.path().join("project/old/block.md"), "later authored edit").unwrap();
            let current = snapshot(dir.path(), &plan.watches).unwrap();
            assert_eq!(recover(dir.path(), rollback, None).unwrap_err().kind(), "recovery_conflict");
            assert_eq!(snapshot(dir.path(), &plan.watches).unwrap(), current);
        }
        let (dir, plan) = fixture();
        apply(dir.path(), &plan, Some(0)).unwrap_err();
        fs::write(dir.path().join("project/additional.md"), "later addition").unwrap();
        assert_eq!(recover(dir.path(), true, None).unwrap_err().kind(), "recovery_conflict");
    }

    #[test]
    fn a_second_writer_and_a_wrong_worktree_are_refused() {
        let (dir, plan) = fixture();
        let _lock = write_lock(dir.path()).unwrap();
        assert_eq!(apply(dir.path(), &plan, None).unwrap_err().kind(), "write_locked");
        let other = tempfile::tempdir().unwrap();
        assert_eq!(apply(other.path(), &plan, None).unwrap_err().kind(), "wrong_worktree");
    }

    #[test]
    fn plan_and_journal_tampering_are_refused() {
        let (dir, mut plan) = fixture();
        plan.operation = "tampered".into();
        assert_eq!(apply(dir.path(), &plan, None).unwrap_err().kind(), "invalid_plan");
        assert!(!dir.path().join(".mind-forge").exists());
        let (dir, plan) = fixture();
        apply(dir.path(), &plan, Some(0)).unwrap_err();
        let mut journal = read_journal(dir.path()).unwrap().unwrap();
        journal.plan.operation = "tampered".into();
        fs::write(dir.path().join(JOURNAL), serde_json::to_vec(&journal).unwrap()).unwrap();
        assert_eq!(recover(dir.path(), false, None).unwrap_err().kind(), "invalid_journal");
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_and_hardlinks_are_rejected_before_backups() {
        let (dir, plan) = fixture();
        std::os::unix::fs::symlink("old/block.md", dir.path().join("project/link.md")).unwrap();
        assert_eq!(apply(dir.path(), &plan, None).unwrap_err().kind(), "unsafe_path");
        let (dir, plan) = fixture();
        fs::hard_link(dir.path().join("project/old/block.md"), dir.path().join("project/link.md")).unwrap();
        assert_eq!(apply(dir.path(), &plan, None).unwrap_err().kind(), "unsafe_path");
    }

    #[test]
    fn interrupted_initial_journal_rename_can_resume_from_durable_next_image() {
        let (dir, plan) = fixture();
        apply(dir.path(), &plan, Some(0)).unwrap_err();
        fs::rename(dir.path().join(JOURNAL), dir.path().join(format!("{JOURNAL}.next"))).unwrap();
        recover(dir.path(), false, None).unwrap();
        assert_eq!(snapshot(dir.path(), &plan.watches).unwrap(), plan.after);
    }
}
