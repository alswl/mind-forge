//! #28 (spec 069): `source new` indexes into RAG in one step.
//!
//! On a Lance-backed repo, adding a source registers it AND chunks/embeds it so
//! it is searchable without a separate `mf source sync`. `--no-index` opts out,
//! indexing failure is best-effort (registration kept + warning), and
//! `--dry-run` writes nothing.

mod common;
use common::embedding_provider::{
    Behavior, KEY_ENV, MockProvider, SECRET, configure_provider, provider_repo_for_embedding, run,
};

/// Default add on a Lance repo with a reachable loopback provider indexes the
/// new source; it is searchable immediately, with no separate sync.
#[test]
fn source_new_indexes_into_rag_without_separate_sync() {
    let mock = MockProvider::start(Behavior::Semantic(384));
    let repo = provider_repo_for_embedding();
    configure_provider(repo.path(), &mock.endpoint); // loopback 127.0.0.1:0

    let newfile = repo.path().join("projects/alpha/extra.md");
    std::fs::write(&newfile, "Photosynthesis converts sunlight into chemical energy inside the chloroplast.\n")
        .unwrap();

    let (out, err, code) = run(
        &repo,
        &["source", "new", newfile.to_str().unwrap(), "--project", "alpha", "--name", "bio"],
        &[(KEY_ENV, SECRET)],
    );
    assert_eq!(code, 0, "source new failed\nstdout:\n{out}\nstderr:\n{err}");
    assert!(out.contains("\"indexed\":true"), "expected add-time indexing\n{out}");
    assert!(mock.request_count() > 0, "add must embed the new source through the provider");

    // Searchable WITHOUT a separate sync.
    let (sout, serr, scode) =
        run(&repo, &["source", "search", "photosynthesis chloroplast", "--mode", "both"], &[(KEY_ENV, SECRET)]);
    assert_eq!(scode, 0, "search failed\nstdout:\n{sout}\nstderr:\n{serr}");
    assert!(
        sout.to_lowercase().contains("chloroplast") || sout.to_lowercase().contains("photosynthesis"),
        "new source must be searchable without sync\n{sout}"
    );
}

/// `--no-index` registers only: no embedding calls, `indexed:false`.
#[test]
fn no_index_registers_without_indexing() {
    let mock = MockProvider::start(Behavior::Vectors(384));
    let repo = provider_repo_for_embedding();
    configure_provider(repo.path(), &mock.endpoint);

    let f = repo.path().join("projects/alpha/skip.md");
    std::fs::write(&f, "content that should not be embedded\n").unwrap();

    let (out, err, code) = run(
        &repo,
        &["source", "new", f.to_str().unwrap(), "--project", "alpha", "--name", "skip", "--no-index"],
        &[(KEY_ENV, SECRET)],
    );
    assert_eq!(code, 0, "source new --no-index failed\nstdout:\n{out}\nstderr:\n{err}");
    assert!(out.contains("\"indexed\":false"), "no-index must not index\n{out}");
    assert_eq!(mock.request_count(), 0, "no-index must not call the embedding provider");
}

/// A failed embedding endpoint is best-effort: the file and registration are
/// kept, a retry warning is surfaced, and the command still exits 0.
#[test]
fn add_index_failure_keeps_registration_and_warns() {
    let repo = provider_repo_for_embedding();
    // Closed loopback port → connection refused immediately (fast failure).
    configure_provider(repo.path(), "http://127.0.0.1:1/v1/embeddings");

    let f = repo.path().join("projects/alpha/best.md");
    std::fs::write(&f, "best effort content\n").unwrap();

    let (out, err, code) = run(
        &repo,
        &["source", "new", f.to_str().unwrap(), "--project", "alpha", "--name", "best"],
        &[(KEY_ENV, SECRET)],
    );
    assert_eq!(code, 0, "add must succeed despite indexing failure\nstdout:\n{out}\nstderr:\n{err}");
    assert!(
        out.contains("source sync") || err.contains("source sync"),
        "expected a retry warning pointing at `mf source sync`\nstdout:\n{out}\nstderr:\n{err}"
    );

    let (lout, lerr, lcode) = run(&repo, &["source", "list", "--project", "alpha"], &[]);
    assert_eq!(lcode, 0, "list failed\n{lerr}");
    assert!(lout.contains("best"), "the registration must persist despite indexing failure\n{lout}");
}

/// `--dry-run source new` writes nothing: no copy, no registration, no embed.
#[test]
fn dry_run_source_new_writes_nothing() {
    let mock = MockProvider::start(Behavior::Vectors(384));
    let repo = provider_repo_for_embedding();
    configure_provider(repo.path(), &mock.endpoint);

    let f = repo.path().join("projects/alpha/preview.md");
    std::fs::write(&f, "dry run content\n").unwrap();

    let (out, err, code) = run(
        &repo,
        &["source", "new", f.to_str().unwrap(), "--project", "alpha", "--name", "previewsrc", "--dry-run"],
        &[(KEY_ENV, SECRET)],
    );
    assert_eq!(code, 0, "dry-run failed\nstdout:\n{out}\nstderr:\n{err}");
    assert_eq!(mock.request_count(), 0, "dry-run must not embed");
    assert!(
        !repo.path().join("projects/alpha/sources/file/preview.md").exists(),
        "dry-run must not copy the file into sources/"
    );

    let (lout, _lerr, _lcode) = run(&repo, &["source", "list", "--project", "alpha"], &[]);
    assert!(!lout.contains("previewsrc"), "dry-run must not register the source\n{lout}");
}
