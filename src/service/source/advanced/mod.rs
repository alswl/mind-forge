//! Advanced Source services — LanceDB-backed repository-level Sources.
//!
//! This module is only active when the repository backend is `lance`.
//! In `legacy` mode, project `mind-index.yaml.sources` remains the
//! authoritative store and these services are not invoked.

#![allow(dead_code)]

pub mod acquisition;
pub mod activation;
pub mod bundle;
pub mod catalog;
pub mod chunk;
pub mod compatibility;
pub mod config;
pub mod embedding;
pub mod enrichment;
pub mod export;
pub mod extraction;
pub mod identity;
pub mod import;
pub mod lance_store;
pub mod lifecycle;
pub mod primary;
pub mod publication;
pub mod retrieval;
pub mod skill_install;
pub mod status;
pub mod sync;
pub mod trace;

use std::path::{Path, PathBuf};

/// mind-forge's per-repository namespace directory. Holds both committed
/// configuration (`renders/`, `publisher/`, `enrichments/`) and gitignored
/// rebuildable runtime state under `cache/`.
pub const MIND_FORGE_DIR: &str = ".mind-forge";
/// Gitignored subdirectory for rebuildable runtime state (indexes, models).
pub const CACHE_DIR: &str = "cache";

/// `<repo>/.mind-forge` — the mind-forge tool namespace root.
pub fn mind_forge_dir(repo_root: &Path) -> PathBuf {
    repo_root.join(MIND_FORGE_DIR)
}

/// `<repo>/.mind-forge/cache` — gitignored, rebuildable runtime state.
pub fn cache_dir(repo_root: &Path) -> PathBuf {
    mind_forge_dir(repo_root).join(CACHE_DIR)
}

/// `<repo>/.mind-forge/cache/source/advanced` — the Lance advanced Source store.
pub fn advanced_store_dir(repo_root: &Path) -> PathBuf {
    cache_dir(repo_root).join("source").join("advanced")
}

/// `<repo>/.mind-forge/enrichments` — committed, human-reviewable Claude
/// enrichments. Unlike the Lance store this is durable authority, not cache:
/// it survives cache deletion and is the source rebuilds restore from.
pub fn enrichments_dir(repo_root: &Path) -> PathBuf {
    mind_forge_dir(repo_root).join("enrichments")
}
