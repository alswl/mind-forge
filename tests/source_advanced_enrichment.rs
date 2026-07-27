//! Historical enrichment CLI is intentionally absent from the unified surface.

mod common;
use common::embedding_provider::{provider_repo, run};

#[test]
fn enrichment_cli_is_removed() {
    let repo = provider_repo();
    let (stdout, stderr, code) = run(&repo, &["source", "enrich", "list"], &[]);
    assert_eq!(code, 2);
    assert!(format!("{stdout}{stderr}").contains("unrecognized"));
}
