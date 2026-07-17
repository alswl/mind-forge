//! Advanced Source services — LanceDB-backed repository-level Sources.
//!
//! This module is only active when the repository backend is `lance`.
//! In `legacy` mode, project `mind-index.yaml.sources` remains the
//! authoritative store and these services are not invoked.

#![allow(dead_code)]

pub mod acquisition;
pub mod activation;
pub mod catalog;
pub mod chunk;
pub mod compatibility;
pub mod config;
pub mod embedding;
pub mod enrichment;
pub mod extraction;
pub mod identity;
pub mod lance_store;
pub mod lifecycle;
pub mod primary;
pub mod publication;
pub mod retrieval;
pub mod skill_install;
pub mod status;
pub mod sync;
