use assert_cmd::Command;

mod common;

// ── Spec 075 US1: Lance storage-schema compatibility (FR-002, FR-007, FR-010) ──
//
// Distinct from the `mind-index.yaml` per-project schema tested above: this
// section covers the *Lance storage* schema — whether the on-disk
// `registrations` table structure matches what this build requires. That
// compatibility is now read from the table itself, never from a declared
// value in `minds.yaml` (spec 075 FR-002).
use common::embedding_provider::{
    Behavior, KEY_ENV, MockProvider, SECRET, configure_provider, provider_repo, provider_repo_for_embedding, report,
    run,
};

/// Rewrite `minds.yaml.source.storage_schema_version`. This key is no longer
/// part of `RepositorySourceConfig` at all — writing it is inert.
fn declare_stale_schema_version_in_yaml(repo: &std::path::Path) {
    let minds = repo.join("minds.yaml");
    let text = std::fs::read_to_string(&minds).expect("read minds.yaml");
    let mut root: serde_yaml::Value = serde_yaml::from_str(&text).expect("parse minds.yaml");
    let source = root
        .as_mapping_mut()
        .and_then(|m| m.get_mut(serde_yaml::Value::String("source".into())))
        .and_then(|s| s.as_mapping_mut())
        .expect("source block");
    source.insert("storage_schema_version".into(), serde_yaml::Value::String("1".into()));
    std::fs::write(&minds, serde_yaml::to_string(&root).unwrap()).expect("write minds.yaml");
}

/// T010: compatibility follows the tables' actual structure. A repo whose
/// tables are current passes regardless of what `minds.yaml` claims.
#[test]
fn search_ignores_a_hand_edited_schema_declaration() {
    let repo = provider_repo();
    let (o, e, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "activation sync failed\n{o}\n{e}");

    declare_stale_schema_version_in_yaml(repo.path());

    let (stdout, stderr, code) = run(&repo, &["source", "search", "notes"], &[]);
    assert_eq!(code, 0, "search must be unaffected by a hand-edited declaration\nstdout:\n{stdout}\nstderr:\n{stderr}");
}

/// T011 (FR-010, SC-002): the previously-documented recovery of hand-editing
/// `storage_schema_version` to `"2"` to unblock search must have nothing left
/// to do — because there is no refusal to escape in the first place once the
/// tables are actually current. This is the CLI-observable half of FR-010;
/// a *genuinely* older table (missing v2 columns) cannot be constructed
/// through the CLI/subprocess boundary at all — that structural refusal is
/// covered directly in `src/service/source/advanced/lance_store.rs`
/// (`registrations_schema_status_reads_actual_table_structure`), which builds
/// a real v1-shaped table and asserts `SchemaStatus::Older`.
#[test]
fn incremental_sync_ignores_a_hand_edited_schema_declaration() {
    let repo = provider_repo();
    let (o, e, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "activation sync failed\n{o}\n{e}");

    declare_stale_schema_version_in_yaml(repo.path());

    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--offline"], &[]);
    assert_eq!(code, 0, "sync must be unaffected by a hand-edited declaration\nstdout:\n{stdout}\nstderr:\n{stderr}");
}

/// T013 (FR-007): after a rebuild where some registrations genuinely fail
/// (a real provider error, not a fake declaration), schema compatibility
/// still holds and `search` succeeds on the first attempt — the tables were
/// rebuilt regardless of per-item outcomes.
#[test]
fn rebuild_with_real_registration_failures_still_leaves_search_working() {
    let mock = MockProvider::start(Behavior::HttpError(500));
    // Registrations already exist; derived (chunk/embedding) data is cleared,
    // so `--rebuild` must actually attempt to re-embed everything and hit the
    // bad provider on every item.
    let repo = provider_repo_for_embedding();
    configure_provider(repo.path(), &mock.endpoint);

    let (stdout, stderr, code) = run(&repo, &["source", "sync", "--rebuild"], &[(KEY_ENV, SECRET)]);
    assert_eq!(code, 0, "rebuild must succeed despite per-item failures\nstdout:\n{stdout}\nstderr:\n{stderr}");
    let r = report(&stdout);
    assert!(
        r["registrations_failed"].as_u64().unwrap_or(0) >= 1,
        "precondition: a real failure must be present\n{stdout}"
    );

    let (stdout, stderr, code) = run(&repo, &["source", "search", "notes", "--mode", "basic"], &[]);
    assert_eq!(code, 0, "search must succeed on the first attempt after rebuild\nstdout:\n{stdout}\nstderr:\n{stderr}");
}

fn setup() -> common::TempDir {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    repo
}

fn mf(repo: &common::TempDir) -> Command {
    let mut command = Command::cargo_bin("mf").unwrap();
    command.args(["--root", repo.path().to_str().unwrap(), "--project", "alpha"]);
    command
}

#[test]
fn unsupported_index_schema_is_usage_error_before_registration() {
    let repo = setup();
    let project = repo.path().join("alpha");
    std::fs::write(project.join("mind-index.yaml"), "schema: '99'\nsources: []\n").unwrap();
    std::fs::create_dir_all(project.join("sources/file")).unwrap();
    let input = project.join("sources/file/existing.md");
    std::fs::write(&input, "existing\n").unwrap();
    let output = mf(&repo).args(["source", "new", input.to_str().unwrap(), "--register-only"]).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("incompatible schema") || stderr.contains("upgrade"), "{stderr}");
    assert_eq!(std::fs::read_to_string(project.join("mind-index.yaml")).unwrap(), "schema: '99'\nsources: []\n");
}

#[test]
fn register_only_preserves_terms_publish_records_and_unknown_source_kind() {
    let repo = setup();
    let project = repo.path().join("alpha");
    std::fs::write(
        project.join("mind-index.yaml"),
        "schema: '1'\nsources:\n  old:\n    type: file\n    path: sources/file/old.md\n    source_kind: article_prompt\nterms:\n  - term: stable\n    pinyin: stable\n    definition: keep\npublish_records:\n  - path: docs/post.md\n    target_name: local\n    status: draft\n",
    )
    .unwrap();
    std::fs::create_dir_all(project.join("sources/file")).unwrap();
    let input = project.join("sources/file/new.md");
    std::fs::write(&input, "new\n").unwrap();
    let output = mf(&repo)
        .args(["source", "new", input.to_str().unwrap(), "--register-only", "--name", "new"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let index = std::fs::read_to_string(project.join("mind-index.yaml")).unwrap();
    assert!(index.contains("article_prompt"));
    assert!(index.contains("stable"));
    assert!(index.contains("publish_records"));
    assert!(index.contains("name: new"));
}
