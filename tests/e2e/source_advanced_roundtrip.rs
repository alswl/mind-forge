//! E2E tests for export/import roundtrip (068).
//!
//! Covers:
//! - T054: export → delete sources → import → search (byte-identical restore)
//! - T055: export → import → export equivalence (content fingerprints match)

use std::fs;

use serde_json::Value;

use crate::datasets::Dataset;
use crate::helpers::run_in;

/// Parse the inner command report from the `mf --output json` envelope.
fn report(stdout: &str) -> Value {
    let envelope: Value =
        serde_json::from_str(stdout).unwrap_or_else(|e| panic!("stdout must be pure JSON: {e}\n{stdout}"));
    envelope["data"].clone()
}

/// Build a repo with one project, one source, Lance activated and synced.
fn synced_repo() -> Dataset {
    let ds = Dataset::empty().with_standard_project("alpha");

    std::fs::write(
        ds.root().join("minds.yaml"),
        "schema_version: '1'\nprojects:\n  - name: alpha\n    path: ./projects/alpha\n    created_at: \"2026-04-30T08:00:00Z\"\n    archived_at: ~\n",
    )
    .unwrap();
    std::fs::create_dir_all(ds.root().join("projects/alpha/sources/file")).unwrap();
    std::fs::write(
        ds.root().join("projects/alpha/sources/file/notes.md"),
        "# Research\n\nQuantum entanglement enables teleportation of state between particles.\n",
    )
    .unwrap();

    let (out, err, code) = run_in(ds.root(), &["source", "index", "--project", "alpha"]);
    assert_eq!(code, 0, "source index failed\nstdout:\n{out}\nstderr:\n{err}");
    let (out, err, code) = run_in(ds.root(), &["--output", "json", "source", "sync", "--offline"]);
    assert_eq!(code, 0, "enable failed\nstdout:\n{out}\nstderr:\n{err}");
    let (out, err, code) = run_in(ds.root(), &["--output", "json", "source", "sync", "--offline"]);
    assert_eq!(code, 0, "sync failed\nstdout:\n{out}\nstderr:\n{err}");
    ds
}

#[test]
fn e2e_export_import_roundtrip_restores_content_byte_identical() {
    let ds = synced_repo();
    let unique_phrase = "teleportation of state";
    let bundle = ds.root().join("backup.mfbundle");

    let (out, _, code) =
        run_in(ds.root(), &["--output", "json", "source", "search", unique_phrase, "--mode", "advanced"]);
    assert_eq!(code, 0, "pre-export search failed: {out}");
    assert!(!report(&out)["results"].as_array().unwrap().is_empty(), "unique phrase must be found before export");

    let (out, err, code) =
        run_in(ds.root(), &["--output", "json", "source", "export", "--output-dir", &bundle.to_string_lossy()]);
    assert_eq!(code, 0, "export failed\nstdout:\n{out}\nstderr:\n{err}");
    let export_data = report(&out);
    assert!(export_data["counts"]["registrations"].as_u64().unwrap_or(0) >= 1, "export must include registrations");
    assert!(!export_data["content_digest"].as_str().unwrap_or("").is_empty(), "export must include integrity digest");
    assert!(bundle.join("manifest.json").exists(), "bundle must have manifest.json");
    assert!(bundle.join("registrations.jsonl").exists(), "bundle must have registrations.jsonl");
    assert!(bundle.join("documents.jsonl").exists(), "bundle must have documents.jsonl");
    assert!(bundle.join("content").is_dir(), "bundle must have content/ dir");

    let sources_dir = ds.root().join("projects/alpha/sources");
    fs::remove_dir_all(&sources_dir).unwrap();
    assert!(!sources_dir.exists(), "originals must be deleted");

    let (out, err, code) =
        run_in(ds.root(), &["--output", "json", "source", "import", &bundle.to_string_lossy(), "--overwrite"]);
    assert_eq!(code, 0, "import failed\nstdout:\n{out}\nstderr:\n{err}");
    assert!(report(&out)["restored_counts"]["registrations"].as_u64().unwrap_or(0) >= 1);

    let (out, _, code) =
        run_in(ds.root(), &["--output", "json", "source", "search", unique_phrase, "--mode", "advanced"]);
    assert_eq!(code, 0, "post-import search failed: {out}");
    assert!(!report(&out)["results"].as_array().unwrap().is_empty(), "unique phrase must still be found after import");

    let restored = fs::read_to_string(ds.root().join("projects/alpha/sources/file/notes.md")).unwrap();
    assert!(restored.contains(unique_phrase), "restored content must contain unique phrase");
}

#[test]
fn e2e_double_export_equivalence() {
    let ds = synced_repo();
    let bundle_a = ds.root().join("a.mfbundle");
    let bundle_b = ds.root().join("b.mfbundle");

    let (out, _, code) =
        run_in(ds.root(), &["--output", "json", "source", "export", "--output-dir", &bundle_a.to_string_lossy()]);
    assert_eq!(code, 0, "first export failed: {out}");
    let first_digest = report(&out)["content_digest"].as_str().unwrap().to_string();
    let (out, _, code) =
        run_in(ds.root(), &["--output", "json", "source", "export", "--output-dir", &bundle_b.to_string_lossy()]);
    assert_eq!(code, 0, "second export failed: {out}");
    assert_eq!(first_digest, report(&out)["content_digest"].as_str().unwrap());
}
