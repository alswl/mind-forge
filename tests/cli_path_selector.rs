//! A path-form positional argument selects its own project, so commands work
//! from the repo root or from inside an article directory without `-p`.

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str;
use std::fs;
use std::path::{Path, PathBuf};

const PROJECT: &str = "projects/2026-blog";

fn setup() -> (tempfile::TempDir, PathBuf) {
    let repo = tempfile::TempDir::new().unwrap();
    fs::write(repo.path().join("minds.yaml"), format!("schema: '1'\nprojects:\n  - {PROJECT}\n")).unwrap();
    let project = repo.path().join(PROJECT);
    fs::create_dir_all(project.join("docs/ai-native-work")).unwrap();
    fs::create_dir_all(project.join("assets")).unwrap();
    fs::create_dir_all(project.join("sources")).unwrap();
    fs::write(project.join("mind.yaml"), "schema: '1'\n").unwrap();
    fs::write(project.join("docs/ai-native-work/01-opening.md"), "Opening\n").unwrap();
    fs::write(project.join("assets/logo.png"), "png").unwrap();
    fs::write(project.join("sources/note.md"), "note\n").unwrap();
    for sub in ["article", "asset", "source"] {
        mf(repo.path(), &[sub, "index", "-p", PROJECT]).assert().success();
    }
    (repo, project)
}

fn mf(cwd: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::cargo_bin("mf").expect("binary exists");
    cmd.current_dir(cwd).args(args);
    cmd
}

#[test]
fn build_accepts_repo_relative_path_from_repo_root_and_from_article_dir() {
    let (repo, project) = setup();
    let output = project.join("outputs/ai-native-work.md");
    let article = project.join("docs/ai-native-work");

    for (cwd, arg) in [(repo.path().to_path_buf(), format!("{PROJECT}/docs/ai-native-work")), (article, ".".into())] {
        mf(&cwd, &["build", &arg]).assert().success().stdout(str::contains("Article built:"));
        assert_eq!(fs::read_to_string(&output).unwrap(), "Opening\n");
        fs::remove_file(&output).unwrap();
    }
}

#[test]
fn article_commands_accept_repo_relative_path() {
    let (repo, _project) = setup();
    let article = format!("{PROJECT}/docs/ai-native-work");

    mf(repo.path(), &["article", "show", &article]).assert().success().stdout(str::contains("docs/ai-native-work"));
    mf(repo.path(), &["article", "block", "renumber", &article, "--dry-run"]).assert().success();
    mf(repo.path(), &["render", &article]).assert().failure().stderr(str::contains("output not found"));
}

#[test]
fn asset_and_source_commands_accept_repo_relative_path() {
    let (repo, _project) = setup();

    mf(repo.path(), &["asset", "show", &format!("{PROJECT}/assets/logo.png")])
        .assert()
        .success()
        .stdout(str::contains("assets/logo.png"));
    mf(repo.path(), &["source", "show", &format!("{PROJECT}/sources/note.md")])
        .assert()
        .success()
        .stdout(str::contains("sources/note.md"));
}

#[test]
fn explicit_project_flag_wins_over_path_discovery() {
    let (repo, _project) = setup();

    mf(repo.path(), &["article", "show", "-p", "2026-blog", &format!("{PROJECT}/docs/ai-native-work")])
        .assert()
        .failure()
        .stderr(str::contains("not found"));
}

#[test]
fn bare_slug_is_not_treated_as_a_path() {
    let (repo, _project) = setup();
    // A directory named like the slug exists in the cwd; a bare slug must still
    // go through project detection rather than be read as that directory.
    fs::create_dir_all(repo.path().join("ai-native-work")).unwrap();

    mf(repo.path(), &["article", "show", "ai-native-work"])
        .assert()
        .failure()
        .stderr(str::contains("could not detect current project"));
}

#[test]
fn path_outside_any_project_keeps_the_original_error() {
    let (repo, _project) = setup();
    fs::create_dir_all(repo.path().join("scratch/docs")).unwrap();

    mf(repo.path(), &["article", "show", "scratch/docs"])
        .assert()
        .failure()
        .stderr(str::contains("could not detect current project").and(str::contains("inside a project")));
}
