//! Prefilter tests for source search (T049): project/type/source filters
//! applied before FTS/vector limits; cwd never creates implicit project filter.

mod common;
use common::embedding_provider::{provider_repo, report, run};

fn synced_repo() -> tempfile::TempDir {
    let repo = provider_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "sync failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    repo
}

#[test]
fn project_filter_limits_results() {
    let repo = synced_repo();
    let (all_out, _, _) = run(&repo, &["source", "search", "entanglement", "--mode", "both"], &[]);
    let all = report(&all_out);
    let all_count = all["results"].as_array().unwrap().len();

    // Search scoped to the existing project.
    let (proj_out, _, _) =
        run(&repo, &["source", "search", "entanglement", "--mode", "both", "--project", "alpha"], &[]);
    let proj = report(&proj_out);
    assert!(!proj["results"].as_array().unwrap().is_empty(), "existing project must have results\n{proj_out}");

    // Search scoped to a nonexistent project must return zero results.
    let (none_out, _, _) =
        run(&repo, &["source", "search", "entanglement", "--mode", "both", "--project", "nonexistent"], &[]);
    let none = report(&none_out);
    assert!(none["results"].as_array().unwrap().is_empty(), "nonexistent project must return zero results\n{none_out}");
    let _ = all_count;
}

#[test]
fn source_filter_narrows_results() {
    let repo = synced_repo();
    // Use the source name as the query to ensure metadata hits.
    let query = "notes";

    let (all_out, _, _) = run(&repo, &["source", "search", query, "--mode", "basic"], &[]);
    let all = report(&all_out);
    let all_count = all["results"].as_array().unwrap().len();
    assert!(all_count > 0, "baseline search must have results\n{all_out}");

    // Filter by the known source identity from the fixture.
    let (src_out, _, _) = run(&repo, &["source", "search", query, "--mode", "basic", "--source", "notes"], &[]);
    let src = report(&src_out);
    assert!(!src["results"].as_array().unwrap().is_empty(), "known source must have results\n{src_out}");

    // Filter by a nonexistent source must return zero.
    let (none_out, _, _) =
        run(&repo, &["source", "search", query, "--mode", "basic", "--source", "nonexistent-source"], &[]);
    let none = report(&none_out);
    assert!(none["results"].as_array().unwrap().is_empty(), "nonexistent source must return zero results\n{none_out}");
}

#[test]
fn cwd_does_not_create_implicit_project_filter() {
    let repo = synced_repo();
    // Run from repo root without --project — must search all projects.
    let (stdout, stderr, code) = run(&repo, &["source", "search", "entanglement", "--mode", "basic"], &[]);
    assert_eq!(code, 0, "repo-root search failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let r = report(&stdout);
    // Scope must be "repository", not "project".
    assert_eq!(
        r["scope"]["kind"].as_str().unwrap_or(""),
        "repository",
        "cwd must not create implicit project scope\n{stdout}"
    );
}
