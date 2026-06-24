use assert_cmd::Command;

mod common;

/// Helper: run a command and return (parsed_json, stderr, exit_code).
///
/// Errors are emitted on stderr as JSON envelopes, so we try stderr first
/// and fall back to stdout for success responses.
fn json_run(args: &[&str], cwd: &std::path::Path) -> (serde_json::Value, String, Option<i32>) {
    let output =
        Command::cargo_bin("mf").expect("binary exists").current_dir(cwd).args(args).output().expect("command runs");
    let code = output.status.code();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let body = if code != Some(0) && !stderr.is_empty() { &stderr } else { &stdout };
    let parsed = serde_json::from_str(body).unwrap_or_else(|_| serde_json::Value::String(body.clone()));
    (parsed, stderr, code)
}

// ---------------------------------------------------------------------------
// T016a: End-to-end generated publish (dry-run)
// ---------------------------------------------------------------------------

#[test]
fn end_to_end_generated_publish() {
    let dest = std::env::temp_dir().join("mf-test-gen-pub");
    let _ = std::fs::remove_dir_all(&dest);

    let repo = common::setup_repo();
    let project_path = repo.path().join("my-project");
    std::fs::create_dir_all(&project_path).unwrap();

    // Write mind.yaml with template + publish target
    let target_path = dest.to_string_lossy().replace('\\', "/");
    let targets_yaml = format!(
        "    - name: local-out\n      type: local\n      enabled: true\n      path: \"{target_path}/{{date:YYYY-MM}}/\"\n      prefix: \"gen-\"\n",
    );
    let mind_yaml = format!(
        "schema_version: '1'\n\
         project:\n  name: my-project\n\
         build:\n  output_dir: _build\n  format: md\n\
         publish:\n  targets:\n{targets_yaml}\n\
         templates:\n  daily_report:\n    pattern: \"outputs/{{date:YYYY-MM}}/{{date:YYYY-MM-DD}}.md\"\n    mode: generated\n",
    );
    common::write_mind_yaml(&repo, "my-project", &mind_yaml);
    common::write_index(&repo, "my-project", "schema_version: '1'\narticles: []\n");

    // Create build artifact (publish locates artifact by article ID: _build/{article}.{format})
    let build_dir = project_path.join("_build/daily_report");
    std::fs::create_dir_all(&build_dir).unwrap();
    std::fs::write(build_dir.join("2026-05-15.md"), b"# Generated content\n").unwrap();

    // Create the generated article file on disk
    let output_dir = project_path.join("outputs/2026-05");
    std::fs::create_dir_all(&output_dir).unwrap();
    std::fs::write(output_dir.join("2026-05-15.md"), b"# Generated content\n").unwrap();

    // Dry-run publish using the generated article ID
    let (parsed, stderr, code) = json_run(
        &["--output", "json", "publish", "run", "daily_report/2026-05-15", "--target", "local-out", "--dry-run"],
        project_path.as_path(),
    );
    assert_eq!(code, Some(0), "dry-run publish of generated article should succeed: stderr={stderr} parsed={parsed}",);

    assert_eq!(parsed["status"], "ok", "envelope should be ok: {parsed}");
    let data = &parsed["data"];
    // The destination should contain the expanded path and prefix
    let destination = data["destination"].as_str().unwrap_or("");
    assert!(destination.contains("gen-"), "destination should contain prefix: {destination}");
    assert!(destination.contains("2026-05"), "destination should contain expanded date: {destination}");
    assert!(destination.contains("2026-05-15"), "should contain article date stem: {destination}");

    let _ = std::fs::remove_dir_all(&dest);
}

// ---------------------------------------------------------------------------
// T016b: Publish auto-reindexes on cache miss
// ---------------------------------------------------------------------------

#[test]
fn publish_auto_reindexes_on_miss() {
    let dest = std::env::temp_dir().join("mf-test-auto-reindex");
    let _ = std::fs::remove_dir_all(&dest);

    let repo = common::setup_repo();
    let project_path = repo.path().join("my-project");
    std::fs::create_dir_all(&project_path).unwrap();

    let target_path = dest.to_string_lossy().replace('\\', "/");
    let targets_yaml = format!(
        "    - name: local-out\n      type: local\n      enabled: true\n      path: \"{target_path}/\"\n      prefix: \"\"\n",
    );
    let mind_yaml = format!(
        "schema_version: '1'\n\
         project:\n  name: my-project\n\
         build:\n  output_dir: _build\n  format: md\n\
         publish:\n  targets:\n{targets_yaml}\n\
         templates:\n  daily_report:\n    pattern: \"outputs/{{date:YYYY-MM-DD}}.md\"\n    mode: generated\n",
    );
    common::write_mind_yaml(&repo, "my-project", &mind_yaml);

    // Delete any prior index — publish must auto-reindex
    let index_path = project_path.join("mind-index.yaml");
    let _ = std::fs::remove_file(&index_path);

    // Create build artifact (needed for publish to locate the artifact)
    let build_dir = project_path.join("_build/daily_report");
    std::fs::create_dir_all(&build_dir).unwrap();
    std::fs::write(build_dir.join("2026-05-16.md"), b"# Auto-reindex\n").unwrap();

    // Create the generated file on disk
    let output_dir = project_path.join("outputs");
    std::fs::create_dir_all(&output_dir).unwrap();
    std::fs::write(output_dir.join("2026-05-16.md"), b"# Auto-reindex\n").unwrap();

    // Publish without prior index — should auto-reindex and succeed
    let (parsed, stderr, code) = json_run(
        &["--output", "json", "publish", "run", "daily_report/2026-05-16", "--target", "local-out", "--dry-run"],
        project_path.as_path(),
    );
    assert_eq!(code, Some(0), "publish should auto-reindex on cache miss: stderr={stderr} parsed={parsed}",);
    assert_eq!(parsed["status"], "ok");

    let _ = std::fs::remove_dir_all(&dest);
}

// ---------------------------------------------------------------------------
// T016c: Publish still missing after reindex
// ---------------------------------------------------------------------------

#[test]
fn publish_still_missing_after_reindex() {
    let repo = common::setup_repo();
    let project_path = repo.path().join("my-project");
    std::fs::create_dir_all(&project_path).unwrap();

    let mind_yaml = "schema_version: '1'\n\
         project:\n  name: my-project\n\
         build:\n  output_dir: _build\n  format: md\n\
         publish:\n  targets:\n    - name: local-out\n      type: local\n      enabled: true\n      path: \"/tmp/mf-miss/\"\n      prefix: \"\"\n\
         templates:\n  daily_report:\n    pattern: \"outputs/{date:YYYY-MM-DD}.md\"\n    mode: generated\n";
    common::write_mind_yaml(&repo, "my-project", mind_yaml);

    // Create artifact for a different article (not the one we'll request)
    let build_dir = project_path.join("_build/daily_report");
    std::fs::create_dir_all(&build_dir).unwrap();
    std::fs::write(build_dir.join("2026-05-17.md"), b"# Some other article\n").unwrap();

    // Create a DIFFERENT generated file
    let output_dir = project_path.join("outputs");
    std::fs::create_dir_all(&output_dir).unwrap();
    std::fs::write(output_dir.join("2026-05-17.md"), b"# Some other article\n").unwrap();

    // Request a non-existent generated article ID
    let (parsed, stderr, code) = json_run(
        &["--output", "json", "publish", "run", "daily_report/9999-01-01", "--target", "local-out", "--dry-run"],
        project_path.as_path(),
    );
    assert_ne!(code, Some(0), "publish of non-existent generated article should fail: {stderr}");
    assert_eq!(parsed["error"]["kind"], "not_found", "should report not_found: {parsed}");
}

// ---------------------------------------------------------------------------
// US3: Generated publish source regression — source is the template file (T029)
// ---------------------------------------------------------------------------

#[test]
fn generated_publish_source_is_template_file() {
    let repo = common::scaffold_team_reports_minimal_repro();
    let project_path = repo.path().join("team-reports");

    let (_, stderr, code) = json_run(&["article", "index"], project_path.as_path());
    assert_eq!(code, Some(0), "index: stderr={stderr}");

    let (parsed, stderr, code) = json_run(
        &["--output", "json", "publish", "run", "daily_report/2026-05-15", "--target", "local-test", "--dry-run"],
        project_path.as_path(),
    );
    assert_eq!(code, Some(0), "publish generated should succeed: stderr={stderr}");
    let source = parsed["data"]["source"].as_str().unwrap_or("");
    assert!(
        source.ends_with("outputs/2026-05/2026-05-15.md"),
        "source should be the generated template article file, got: {source}"
    );
}

// ---------------------------------------------------------------------------
// US3: Generated publish date expansion and prefix in destination (T030)
// ---------------------------------------------------------------------------

#[test]
fn generated_publish_date_expansion_and_prefix_destination() {
    let repo = common::scaffold_team_reports_minimal_repro();
    let project_path = repo.path().join("team-reports");

    let (_, stderr, code) = json_run(&["article", "index"], project_path.as_path());
    assert_eq!(code, Some(0), "index: stderr={stderr}");

    let (parsed, stderr, code) = json_run(
        &["--output", "json", "publish", "run", "daily_report/2026-05-15", "--target", "local-test", "--dry-run"],
        project_path.as_path(),
    );
    assert_eq!(code, Some(0), "publish generated should succeed: stderr={stderr}");
    let dest = parsed["data"]["destination"].as_str().unwrap_or("");
    assert!(
        dest.contains("/2026-05/daily/cie-2026-05-15.md"),
        "destination should expand date placeholder and apply prefix, got: {dest}"
    );
}
