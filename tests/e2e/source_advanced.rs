//! E2E tests for advanced Source workflows (LanceDB-backed Sources).
//!
//! Covers the US1 → US2 main path offline (no embedding model required —
//! chunk vectors are zero-filled until an explicit model pipeline runs, and
//! keyword/FTS retrieval still returns real results):
//!
//! 1. `source advanced enable` imports every legacy registration and switches
//!    the repository to the Lance backend.
//! 2. `source advanced sync --offline` persists shared content and chunks.
//! 3. `source search` finds that content repository-wide, including from a
//!    different project's working directory (one shared corpus).
//! 4. `source advanced status` reports the active Lance backend.

use serde_json::Value;

use crate::datasets::Dataset;
use crate::helpers::run_in;

/// minds.yaml registering two active projects, alpha and beta.
const MANIFEST_ALPHA_BETA: &str = "schema_version: '1'\nprojects:\n  \
    - name: alpha\n    path: ./projects/alpha\n    created_at: \"2026-04-30T08:00:00Z\"\n    archived_at: ~\n  \
    - name: beta\n    path: ./projects/beta\n    created_at: \"2026-04-30T09:00:00Z\"\n    archived_at: ~\n";

/// Build a two-project repo with one Markdown Source per project, registered
/// into each project's legacy `mind-index.yaml` via `source index`.
fn indexed_repo() -> Dataset {
    let ds = Dataset::empty().with_standard_project("alpha").with_standard_project("beta");
    std::fs::write(ds.root().join("minds.yaml"), MANIFEST_ALPHA_BETA).expect("write manifest");

    std::fs::create_dir_all(ds.root().join("projects/alpha/sources/file")).expect("alpha source dir");
    std::fs::write(
        ds.root().join("projects/alpha/sources/file/alpha-notes.md"),
        "# Alpha Notes\n\nQuantum entanglement enables teleportation of state.\n",
    )
    .expect("write alpha source");

    std::fs::create_dir_all(ds.root().join("projects/beta/sources/file")).expect("beta source dir");
    std::fs::write(
        ds.root().join("projects/beta/sources/file/beta-notes.md"),
        "# Beta Notes\n\nPhotosynthesis converts light in the chloroplast.\n",
    )
    .expect("write beta source");

    for project in ["alpha", "beta"] {
        let (stdout, stderr, code) = run_in(ds.root(), &["source", "index", "--project", project]);
        assert_eq!(code, 0, "source index {project} failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    }
    ds
}

/// Parse the inner command report from the `mf --output json` envelope.
///
/// stdout is a machine-readable JSON contract: dependency logs (LanceDB) go to
/// stderr, so stdout must parse cleanly with the report at `data.data`.
fn report(stdout: &str) -> Value {
    let envelope: Value =
        serde_json::from_str(stdout).unwrap_or_else(|e| panic!("stdout must be pure JSON: {e}\n{stdout}"));
    envelope["data"]["data"].clone()
}

#[test]
fn enable_imports_legacy_registrations_and_activates_lance() {
    let ds = indexed_repo();

    let (stdout, stderr, code) = run_in(ds.root(), &["--output", "json", "source", "advanced", "enable"]);
    assert_eq!(code, 0, "enable failed\nstdout:\n{stdout}\nstderr:\n{stderr}");

    let data = report(&stdout);
    assert_eq!(data["total_registrations"], 2, "enable must import both legacy registrations: {stdout}");

    // The backend marker is now Lance; status reflects it.
    let (status_out, _, status_code) = run_in(ds.root(), &["--output", "json", "source", "advanced", "status"]);
    assert_eq!(status_code, 0, "status failed: {status_out}");
    assert_eq!(report(&status_out)["backend"], "lance", "backend marker should switch to lance: {status_out}");
}

#[test]
fn sync_persists_shared_content_offline() {
    let ds = indexed_repo();
    let (out, err, code) = run_in(ds.root(), &["--output", "json", "source", "advanced", "enable"]);
    assert_eq!(code, 0, "enable failed: {out}{err}");

    let (stdout, stderr, code) = run_in(ds.root(), &["--output", "json", "source", "advanced", "sync", "--offline"]);
    assert_eq!(code, 0, "offline sync failed\nstdout:\n{stdout}\nstderr:\n{stderr}");

    let data = report(&stdout);
    assert_eq!(data["registrations_total"], 2, "both registrations processed: {stdout}");
    assert_eq!(data["registrations_added"], 2, "both registrations newly synced: {stdout}");

    let chunks: u64 =
        data["items"].as_array().expect("items array").iter().map(|i| i["affected_chunks"].as_u64().unwrap_or(0)).sum();
    assert!(chunks >= 2, "sync should persist at least one chunk per source: {stdout}");
}

/// Build a repo whose content has already been synced into the Lance corpus.
fn synced_repo() -> Dataset {
    let ds = indexed_repo();
    let (out, err, code) = run_in(ds.root(), &["source", "advanced", "enable"]);
    assert_eq!(code, 0, "enable failed: {out}{err}");
    let (out, err, code) = run_in(ds.root(), &["source", "advanced", "sync", "--offline"]);
    assert_eq!(code, 0, "sync failed: {out}{err}");
    ds
}

#[test]
fn advanced_search_finds_content_from_another_projects_cwd() {
    let ds = synced_repo();

    // Alpha's content is discoverable from beta's working directory — one
    // shared repository-wide corpus, not a per-project index.
    let beta_cwd = ds.root().join("projects/beta");
    let (stdout, stderr, code) =
        run_in(&beta_cwd, &["--output", "json", "source", "search", "quantum entanglement", "--mode", "advanced"]);
    assert_eq!(code, 0, "advanced search failed\nstdout:\n{stdout}\nstderr:\n{stderr}");

    let data = report(&stdout);
    assert_eq!(data["scope"]["kind"], "repository", "default scope is repository-wide: {stdout}");
    let results = data["results"].as_array().expect("results array");
    assert_eq!(results.len(), 1, "exactly alpha's document matches: {stdout}");
    assert_eq!(
        results[0]["registrations"][0]["registered_location"], "sources/file/alpha-notes.md",
        "match should resolve to alpha's registered source: {stdout}"
    );
    assert!(
        results[0]["snippet"].as_str().unwrap_or_default().contains("Quantum"),
        "snippet carries content: {stdout}"
    );
}

#[test]
fn basic_search_matches_metadata_not_content() {
    let ds = synced_repo();

    // Basic mode searches Source metadata (name/tags), not indexed content.
    let (meta_out, _, code) = run_in(ds.root(), &["--output", "json", "source", "search", "beta", "--mode", "basic"]);
    assert_eq!(code, 0, "basic search failed: {meta_out}");
    let meta = report(&meta_out);
    assert_eq!(meta["results"].as_array().map(|r| r.len()), Some(1), "metadata name match: {meta_out}");
    assert!(
        meta["actual_paths"].as_array().expect("paths").iter().any(|p| p == "basic"),
        "basic path used: {meta_out}"
    );

    // A content-only term does not surface via basic metadata search.
    let (content_out, _, code) =
        run_in(ds.root(), &["--output", "json", "source", "search", "photosynthesis", "--mode", "basic"]);
    assert_eq!(code, 0, "basic content-term search failed: {content_out}");
    assert_eq!(
        report(&content_out)["results"].as_array().map(|r| r.len()),
        Some(0),
        "content term must not match in basic mode: {content_out}"
    );
}

#[test]
fn status_reports_lance_backend_and_counts() {
    let ds = synced_repo();

    let (stdout, stderr, code) = run_in(ds.root(), &["--output", "json", "source", "advanced", "status"]);
    assert_eq!(code, 0, "status failed\nstdout:\n{stdout}\nstderr:\n{stderr}");

    let data = report(&stdout);
    assert_eq!(data["backend"], "lance", "backend should be lance: {stdout}");
    assert_eq!(data["registrations_count"], 2, "two registrations in the corpus: {stdout}");
    assert_eq!(data["documents_count"], 2, "two shared documents after sync: {stdout}");
    assert!(data["chunks_count"].as_u64().unwrap_or(0) >= 2, "chunks persisted: {stdout}");
}
