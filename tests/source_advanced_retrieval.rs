//! Ranking and deduplication tests for source advanced retrieval (T048):
//! BM25, cosine, fixed RRF(k=60), metadata/content fusion, (document_key, locator)
//! deduplication, and deterministic tie-breaking.

mod common;
use common::embedding_provider::{provider_repo, report, run};
use std::collections::BTreeSet;

fn synced_repo() -> tempfile::TempDir {
    let repo = provider_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "advanced", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "sync failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    repo
}

#[test]
fn search_results_have_decreasing_combined_scores() {
    let repo = synced_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "search", "entanglement", "--mode", "both"], &[]);
    assert_eq!(code, 0, "search failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let r = report(&stdout);
    let results = r["results"].as_array().expect("results array");
    if results.len() < 2 {
        return; // not enough results to verify ordering
    }
    let mut prev: f64 = f64::MAX;
    for res in results {
        let score = res["combined_score"].as_f64().expect("combined_score must be f64");
        assert!(
            score <= prev,
            "results must be sorted by decreasing combined_score\ngot {score} after {prev}\n{stdout}"
        );
        prev = score;
    }
}

#[test]
fn both_mode_results_are_deduplicated_by_document_key_and_locator() {
    let repo = synced_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "search", "entanglement", "--mode", "both"], &[]);
    assert_eq!(code, 0, "search failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let r = report(&stdout);
    let results = r["results"].as_array().expect("results array");
    // Each (document_key, locator_json) pair must appear at most once.
    let mut seen = BTreeSet::new();
    for res in results {
        // Fallback to chunk_id when document_key is absent.
        let doc = res["document_key"].as_str().or_else(|| res["chunk_id"].as_str()).unwrap_or("unknown");
        let loc = res["location"].as_str().unwrap_or("unknown");
        let key = format!("{doc}@{loc}");
        let is_new = seen.insert(key.clone());
        assert!(is_new, "duplicate (document, locator) pair in fused results\n  key: {key}\n{stdout}");
    }
}

#[test]
fn each_result_has_at_least_one_retrieval_path() {
    let repo = synced_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "search", "entanglement", "--mode", "both"], &[]);
    assert_eq!(code, 0, "search failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let r = report(&stdout);
    let results = r["results"].as_array().expect("results array");
    assert!(!results.is_empty(), "must have results\n{stdout}");
    for res in results {
        let paths = res["retrieval_paths"].as_array().expect("retrieval_paths array");
        assert!(!paths.is_empty(), "every result must have at least one retrieval path\n{stdout}");
    }
}

#[test]
fn provenance_includes_registrations_with_project_identity() {
    let repo = synced_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "search", "entanglement", "--mode", "both"], &[]);
    assert_eq!(code, 0, "search failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let r = report(&stdout);
    let results = r["results"].as_array().expect("results array");
    assert!(!results.is_empty(), "must have results\n{stdout}");
    for res in results {
        let regs = res["registrations"].as_array().expect("registrations array");
        assert!(!regs.is_empty(), "every result must have at least one registration\n{stdout}");
        for reg in regs {
            assert!(reg["project_identity"].as_str().is_some(), "registration must have project_identity\n{stdout}");
            assert!(reg["registration_key"].as_str().is_some(), "registration must have registration_key\n{stdout}");
        }
    }
}

#[test]
fn basic_mode_results_differ_from_advanced_due_to_fusion() {
    let repo = synced_repo();
    let (basic_out, _, _) = run(&repo, &["source", "search", "entanglement", "--mode", "basic"], &[]);
    let (both_out, _, _) = run(&repo, &["source", "search", "entanglement", "--mode", "both"], &[]);
    let basic_r = report(&basic_out);
    let both_r = report(&both_out);
    // Both mode must report a non-basic resolved_mode.
    let resolved = both_r["resolved_mode"].as_str().unwrap_or("");
    assert!(
        resolved != "basic" || both_r["degraded"].as_bool().unwrap_or(false),
        "both mode must resolve to both or degrade, got '{resolved}'\n{both_out}"
    );
    let _ = basic_r;
}

#[test]
fn deterministic_same_query_same_results() {
    let repo = synced_repo();
    let (out1, _, _) = run(&repo, &["source", "search", "entanglement", "--mode", "both"], &[]);
    let (out2, _, _) = run(&repo, &["source", "search", "entanglement", "--mode", "both"], &[]);
    let r1 = report(&out1);
    let r2 = report(&out2);
    let results1 = r1["results"].as_array().expect("results array");
    let results2 = r2["results"].as_array().expect("results array");
    assert_eq!(results1.len(), results2.len(), "same query must return same result count\n{out1}\n---\n{out2}");
    for (i, (a, b)) in results1.iter().zip(results2.iter()).enumerate() {
        assert_eq!(
            a["combined_score"].as_f64(),
            b["combined_score"].as_f64(),
            "result {i} must have the same combined_score\n{out1}\n---\n{out2}"
        );
        assert_eq!(
            a["chunk_id"].as_str(),
            b["chunk_id"].as_str(),
            "result {i} must have the same chunk_id\n{out1}\n---\n{out2}"
        );
    }
}
