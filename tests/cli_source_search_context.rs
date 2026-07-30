//! CLI contract tests for per-hit context on `mf source search` (spec 071, US1).
//!
//! Every search hit's `registrations[]` carries a structured `context`:
//! repository/project attribution, project goal, content kind, lifecycle,
//! relations (resolved + dangling), and — for source bindings — import
//! provenance. Shared source content is listed per binding without duplicating
//! the same content location.

mod common;
use common::embedding_provider::{provider_repo, report, run};

/// Extend the activated provider repo with an article that has a project goal,
/// front-matter status, one resolved and one dangling internal link, and a
/// prompt sibling. Returns the repo after a re-sync so discovery picks it up.
fn repo_with_article() -> tempfile::TempDir {
    let repo = provider_repo();
    let alpha = repo.path().join("projects/alpha");
    std::fs::write(alpha.join("mind.yaml"), "schema_version: '1'\ngoal: Study quantum teleportation\n").unwrap();

    let outputs = alpha.join("docs");
    std::fs::create_dir_all(&outputs).unwrap();
    std::fs::write(
        outputs.join("teleport.md"),
        "---\nstatus: draft\n---\n\n# Teleport\n\nSee [related](./entangle.md) and [missing](./ghost.md).\n\nQuantum teleportation transfers state across space.\n",
    )
    .unwrap();
    // Resolved link target.
    std::fs::write(outputs.join("entangle.md"), "# Entangle\n\nEntanglement basics.\n").unwrap();
    // Prompt sibling by naming convention.
    let prompts = alpha.join("prompts");
    std::fs::create_dir_all(&prompts).unwrap();
    std::fs::write(prompts.join("teleport.md"), "Draft an article on teleportation.\n").unwrap();

    let (o, e, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "resync failed\n{o}\n{e}");
    repo
}

/// Find the first hit whose location matches `needle`.
fn hit_at<'a>(results: &'a serde_json::Value, needle: &str) -> &'a serde_json::Value {
    results
        .as_array()
        .expect("results array")
        .iter()
        .find(|r| r["location"].as_str().is_some_and(|l| l.contains(needle)))
        .unwrap_or_else(|| panic!("no hit matching {needle}\n{results:#}"))
}

#[test]
fn article_hit_carries_full_context() {
    let repo = repo_with_article();
    let (stdout, stderr, code) = run(&repo, &["source", "search", "teleport", "--mode", "basic"], &[]);
    assert_eq!(code, 0, "search failed\n{stdout}\n{stderr}");
    let r = report(&stdout);
    let hit = hit_at(&r["results"], "teleport.md");
    let ctx = &hit["registrations"][0]["context"];

    assert!(ctx["repository"].as_str().is_some(), "repository present\n{ctx:#}");
    assert_eq!(ctx["project_identity"], "alpha");
    assert_eq!(ctx["project_goal"], "Study quantum teleportation");
    assert_eq!(ctx["content_kind"], "article");
    assert_eq!(ctx["lifecycle_status"], "draft");
    assert_eq!(ctx["single_owner"], true);
    assert!(ctx["relations"].as_array().is_some(), "relations array present\n{ctx:#}");
}

#[test]
fn article_relations_mark_resolved_and_dangling() {
    let repo = repo_with_article();
    let (stdout, _e, code) = run(&repo, &["source", "search", "teleport", "--mode", "basic"], &[]);
    assert_eq!(code, 0);
    let r = report(&stdout);
    let hit = hit_at(&r["results"], "teleport.md");
    let relations = hit["registrations"][0]["context"]["relations"].as_array().expect("relations");

    let resolved = relations.iter().find(|rel| rel["target"].as_str() == Some("./entangle.md")).expect("resolved link");
    assert_eq!(resolved["relation_type"], "article_to_article");
    assert_eq!(resolved["resolved"], true);

    let dangling = relations.iter().find(|rel| rel["target"].as_str() == Some("./ghost.md")).expect("dangling link");
    assert_eq!(dangling["resolved"], false, "dangling target must be marked unresolved, not fabricated");

    // Prompt sibling relation is present.
    assert!(
        relations.iter().any(|rel| rel["relation_type"] == "article_to_prompt"),
        "prompt sibling relation must be present\n{relations:#?}"
    );
}

/// Collect (path, len, mtime) for every file under `root`, for a read-only check.
fn snapshot_tree(root: &std::path::Path) -> Vec<(String, u64, std::time::SystemTime)> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let meta = entry.metadata().unwrap();
            if meta.is_dir() {
                stack.push(path);
            } else {
                out.push((path.to_string_lossy().to_string(), meta.len(), meta.modified().unwrap()));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn results_and_relations_order_is_stable() {
    let repo = repo_with_article();
    let (first, _e, c1) = run(&repo, &["source", "search", "teleport", "--mode", "basic"], &[]);
    let (second, _e2, c2) = run(&repo, &["source", "search", "teleport", "--mode", "basic"], &[]);
    assert_eq!(c1, 0);
    assert_eq!(c2, 0);
    // Identical repository facts + query → byte-identical results and relations.
    assert_eq!(report(&first)["results"], report(&second)["results"], "search output must be deterministic");
}

#[test]
fn search_is_read_only() {
    let repo = repo_with_article();
    let before = snapshot_tree(repo.path());
    let (_o, _e, code) = run(&repo, &["source", "search", "teleport", "--mode", "both"], &[]);
    assert_eq!(code, 0);
    let after = snapshot_tree(repo.path());
    assert_eq!(before, after, "search must not modify any file in the repository");
}

#[test]
fn source_hit_context_is_provenance_only() {
    let repo = repo_with_article();
    let (stdout, _e, code) = run(&repo, &["source", "search", "notes", "--mode", "basic"], &[]);
    assert_eq!(code, 0);
    let r = report(&stdout);
    let hit = hit_at(&r["results"], "notes.md");
    let ctx = &hit["registrations"][0]["context"];
    assert_eq!(ctx["content_kind"], "source");
    assert_eq!(ctx["single_owner"], false);
    // Source bindings expose import provenance (project always present).
    assert_eq!(ctx["imported_by"]["project"], "alpha");
}
