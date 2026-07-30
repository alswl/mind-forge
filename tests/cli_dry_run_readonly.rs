//! Table-driven regression coverage for the repository-wide dry-run contract.
//!
//! Each case receives a fresh repository so a command cannot accidentally pass
//! because a previous case left the same target in place.  The assertion is
//! deliberately a byte-for-byte tree comparison, including generated index
//! and local-state files.

use assert_cmd::Command;

mod common;

fn fixture() -> common::TempDir {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    common::create_project(&repo, "beta");
    let alpha = repo.path().join("alpha");
    std::fs::create_dir_all(alpha.join("docs/post")).unwrap();
    std::fs::create_dir_all(alpha.join("sources/file")).unwrap();
    std::fs::create_dir_all(alpha.join("assets")).unwrap();
    std::fs::write(alpha.join("docs/post/01-intro.md"), "# Intro\n").unwrap();
    std::fs::write(alpha.join("docs/post/02-body.md"), "# Body\n").unwrap();
    std::fs::write(alpha.join("sources/file/note.md"), "source\n").unwrap();
    std::fs::write(alpha.join("assets/diagram.png"), b"asset").unwrap();
    std::fs::write(repo.path().join("external.md"), "external\n").unwrap();
    std::fs::write(repo.path().join("external.bin"), b"external").unwrap();
    std::fs::write(
        alpha.join("mind-index.yaml"),
        "schema_version: '1'\narticles:\n  - title: Post\n    project: alpha\n    type: blank\n    article_path: docs/post\n    status: draft\n    created_at: ''\n    updated_at: ''\nsources:\n  - name: note\n    type: file\n    path: sources/file/note.md\nassets:\n  - name: diagram\n    type: image\n    path: assets/diagram.png\n    size: 5\n    hash: ''\n    tags: []\n    added_at: ''\n",
    )
    .unwrap();
    repo
}

#[test]
fn every_mutating_surface_in_the_matrix_is_read_only_under_dry_run() {
    let cases: &[(&str, &[&str])] = &[
        ("project new", &["project", "new", "gamma", "--dry-run"]),
        ("project index", &["project", "index", "--dry-run"]),
        ("project update", &["project", "update", "alpha", "--description", "preview", "--dry-run"]),
        ("project rename", &["project", "rename", "alpha", "gamma", "--dry-run"]),
        ("project remove", &["project", "remove", "alpha", "--yes", "--dry-run"]),
        ("article new", &["article", "new", "Draft", "--project", "alpha", "--dry-run"]),
        ("article index", &["article", "index", "--project", "alpha", "--dry-run"]),
        (
            "article update",
            &["article", "update", "docs/post", "--title", "Preview", "--project", "alpha", "--dry-run"],
        ),
        ("article rename", &["article", "rename", "docs/post", "renamed", "--project", "alpha", "--dry-run"]),
        ("article remove", &["article", "remove", "docs/post", "--project", "alpha", "--yes", "--dry-run"]),
        (
            "article block new",
            &[
                "article",
                "block",
                "new",
                "docs/post",
                "middle",
                "--after",
                "01-intro.md",
                "--project",
                "alpha",
                "--dry-run",
            ],
        ),
        (
            "article block move",
            &[
                "article",
                "block",
                "move",
                "docs/post",
                "body",
                "--after",
                "01-intro.md",
                "--project",
                "alpha",
                "--dry-run",
            ],
        ),
        ("article block renumber", &["article", "block", "renumber", "docs/post", "--project", "alpha", "--dry-run"]),
        ("source new", &["source", "new", "external.md", "--project", "alpha", "--name", "incoming", "--dry-run"]),
        ("source index", &["source", "index", "--project", "alpha", "--dry-run"]),
        ("source rename", &["source", "rename", "note", "renamed", "--project", "alpha", "--dry-run"]),
        ("source remove", &["source", "remove", "note", "--project", "alpha", "--yes", "--force", "--dry-run"]),
        ("source move", &["source", "move", "note", "--to-project", "beta", "--project", "alpha", "--dry-run"]),
        ("source clean", &["source", "clean", "--project", "alpha", "--dry-run"]),
        ("asset new", &["asset", "new", "external.bin", "--name", "incoming", "--project", "alpha", "--dry-run"]),
        ("asset index", &["asset", "index", "--project", "alpha", "--dry-run"]),
        (
            "asset rename",
            &["asset", "rename", "assets/diagram.png", "assets/renamed.png", "--project", "alpha", "--dry-run"],
        ),
        ("asset remove", &["asset", "remove", "diagram", "--project", "alpha", "--yes", "--dry-run"]),
        ("asset move", &["asset", "move", "diagram", "--to-project", "beta", "--project", "alpha", "--dry-run"]),
        ("asset clean", &["asset", "clean", "--project", "alpha", "--dry-run"]),
        ("term new", &["term", "new", "Preview", "--project", "alpha", "--dry-run"]),
        ("build", &["build", "docs/post", "--project", "alpha", "--dry-run"]),
    ];

    for (name, args) in cases {
        let repo = fixture();
        let before = common::snapshot_tree(repo.path());
        let mut command = Command::cargo_bin("mf").unwrap();
        let output =
            command.args(["--root", repo.path().to_str().unwrap()]).args(args.iter().copied()).output().unwrap();
        assert!(
            output.status.success(),
            "{name} dry-run failed (args={args:?}):\nstdout={}\nstderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        common::assert_tree_unchanged(repo.path(), &before);
    }
}
