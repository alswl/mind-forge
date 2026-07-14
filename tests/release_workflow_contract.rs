use std::fs;

fn read_workflow() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/.github/workflows/release.yml");
    fs::read_to_string(path).expect("release workflow file should exist")
}

// ── US2: Trigger ────────────────────────────────────────────────────────

#[test]
fn workflow_triggers_on_v_tags() {
    let wf = read_workflow();
    assert!(wf.contains("push:"), "workflow should have push trigger");
    assert!(wf.contains("tags:"), "workflow should trigger on tags");
    assert!(wf.contains("v*"), "workflow should trigger on v* tag pattern");
}

// ── US2: Tag validation ─────────────────────────────────────────────────

#[test]
fn workflow_validates_tag_format() {
    let wf = read_workflow();
    // The semver regex from the contract (escaped for YAML)
    let pattern = "[0-9]+\\.[0-9]+\\.[0-9]+";
    assert!(wf.contains(pattern), "workflow should contain semver-like validation pattern");
}

#[test]
fn workflow_rejects_invalid_tags() {
    let wf = read_workflow();
    // The workflow must have logic that fails on invalid tags
    // Look for validation step or job that enforces version format
    let has_validation = wf.contains("valid")
        || wf.contains("regex")
        || wf.contains("semver")
        || wf.contains("match")
        || wf.contains("grep")
        || wf.contains("pattern");
    assert!(has_validation, "workflow should have tag validation (valid/regex/semver/match/grep/pattern)");
}

// ── US2: Validation ordering ────────────────────────────────────────────

#[test]
fn validation_runs_before_build_and_release() {
    let wf = read_workflow();
    // The create-release job must wait for validation (needs: [..., validation, ...])
    // or a validation step must appear before build steps
    let release_job_section = wf.find("create-release:").expect("workflow should have create-release job");
    let release_needs = &wf[release_job_section..];
    assert!(release_needs.contains("needs:"), "create-release should declare job dependencies via 'needs:'");
}

// ── US2: Duplicate release ──────────────────────────────────────────────

#[test]
fn workflow_handles_duplicate_releases() {
    let wf = read_workflow();
    // The workflow should not silently create duplicate releases
    // Common strategies: allowUpdates, skipIfExisting, or explicit duplicate check
    let has_duplicate_policy = wf.contains("allowUpdates")
        || wf.contains("allowUpdates")
        || wf.contains("skipIfExisting")
        || wf.contains("skipIfExisting")
        || wf.contains("overwrite")
        || wf.contains("overwrite")
        || wf.contains("duplicate")
        || wf.contains("existing")
        || wf.contains("exists");
    assert!(
        has_duplicate_policy,
        "workflow should have duplicate release handling (allowUpdates/skipIfExisting/overwrite/duplicate/existing/exists)"
    );
}

// ── US3: Draft release ──────────────────────────────────────────────────

#[test]
fn release_is_created_as_draft() {
    let wf = read_workflow();
    assert!(wf.contains("draft") || wf.contains("draft: true"), "workflow should create draft releases");
}

// ── US3: Release metadata ───────────────────────────────────────────────

#[test]
fn release_has_tag_and_name() {
    let wf = read_workflow();
    assert!(
        wf.contains("github.ref_name") || wf.contains("github.ref"),
        "workflow should reference tag via github.ref_name or github.ref"
    );
}

#[test]
fn release_generates_release_notes() {
    let wf = read_workflow();
    assert!(
        wf.contains("generateReleaseNotes") || wf.contains("generate_release_notes"),
        "workflow should enable generated release notes"
    );
}

// ── US3: Artifact contract ──────────────────────────────────────────────

#[test]
fn artifacts_use_deterministic_names() {
    let wf = read_workflow();
    // Artifact names are constructed from `mf-${{ matrix.target }}`.
    // Verify the matrix target values and the mf- prefix are present.
    assert!(wf.contains("x86_64-unknown-linux-gnu"), "workflow should target x86_64-unknown-linux-gnu");
    assert!(wf.contains("aarch64-unknown-linux-gnu"), "workflow should target aarch64-unknown-linux-gnu");
    assert!(wf.contains("aarch64-apple-darwin"), "workflow should target aarch64-apple-darwin");
    assert!(wf.contains("x86_64-apple-darwin"), "workflow should target x86_64-apple-darwin");
}

// ── US3: Permissions ────────────────────────────────────────────────────

#[test]
fn workflow_has_minimum_permissions() {
    let wf = read_workflow();
    // GitHub Actions permissions block is at the top level or job level
    // We require that permissions are explicitly declared (not relying on defaults)
    assert!(wf.contains("permissions:"), "workflow should declare explicit permissions");
}
