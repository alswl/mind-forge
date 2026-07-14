//! Skill bundle installer for /mf-source Claude Code Skill.
//!
//! Installs the canonical `mf-source` Skill into a Mind Repo's
//! `.claude/skills/mf-source/` directory. Never writes to personal
//! `~/.claude/skills`. Verifies digest, detects conflicts, supports
//! dry-run and force overwrite.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{MfError, Result};

/// Path to the bundled Skill in the mf source tree (relative to the binary).
const SKILL_SOURCE_DIR: &str = "skills/mf-source";

/// Target directory within the Mind Repo.
const SKILL_TARGET: &str = ".claude/skills/mf-source";

/// Bundle manifest filename.
const MANIFEST_FILE: &str = "manifest.json";

/// Result of a skill installation.
#[derive(Debug, Serialize)]
pub struct SkillInstallResult {
    pub installed: bool,
    pub target: String,
    pub version: String,
    pub digest: String,
    pub conflict: Option<String>,
    pub dry_run: bool,
}

/// Compute a simple hash of the Skill bundle for digest comparison.
fn bundle_digest(source_dir: &Path) -> Result<String> {
    let manifest_path = source_dir.join(MANIFEST_FILE);
    if manifest_path.exists() {
        let data = fs::read_to_string(&manifest_path)?;
        Ok(crate::service::source::advanced::identity::raw_fingerprint(data.as_bytes()))
    } else {
        Ok("unknown".to_string())
    }
}

/// Install the Skill bundle into the target Mind Repo.
///
/// - `repo_root`: the Mind Repo root directory
/// - `force`: overwrite existing files even if they differ
/// - `dry_run`: report what would happen without writing
pub fn install_skill(repo_root: &Path, force: bool, dry_run: bool) -> Result<SkillInstallResult> {
    let target_dir = repo_root.join(SKILL_TARGET);

    // Locate the source bundle relative to the current executable or repo
    let source_dir = find_skill_source(repo_root)?;
    let digest = bundle_digest(&source_dir)?;
    let manifest = read_manifest_version(&source_dir)?;

    // Check for existing installation
    let existing = target_dir.exists();
    let conflict = if existing && !force {
        let existing_digest = bundle_digest(&target_dir).unwrap_or_default();
        if existing_digest != digest {
            Some(format!(
                "existing Skill differs from the bundled version (existing: {}, bundled: {})",
                &existing_digest[..16],
                &digest[..16]
            ))
        } else {
            None
        }
    } else {
        None
    };

    if dry_run {
        return Ok(SkillInstallResult {
            installed: false,
            target: target_dir.to_string_lossy().to_string(),
            version: manifest,
            digest,
            conflict,
            dry_run: true,
        });
    }

    // If same digest, it's a no-op
    if existing && conflict.is_none() {
        return Ok(SkillInstallResult {
            installed: false,
            target: target_dir.to_string_lossy().to_string(),
            version: manifest,
            digest,
            conflict: None,
            dry_run: false,
        });
    }

    // If conflict without force, refuse
    if let Some(ref conflict_msg) = conflict {
        return Err(MfError::usage(
            format!("Skill installation conflict: {conflict_msg}"),
            Some("use --force to overwrite, or remove the existing Skill manually".to_string()),
        ));
    }

    // Install: copy all files from source to target
    copy_dir(&source_dir, &target_dir)?;

    Ok(SkillInstallResult {
        installed: true,
        target: target_dir.to_string_lossy().to_string(),
        version: manifest,
        digest,
        conflict: None,
        dry_run: false,
    })
}

/// Find the Skill source directory. First checks relative to the repo root,
/// then checks relative to the current working directory.
fn find_skill_source(repo_root: &Path) -> Result<PathBuf> {
    // Check the repo's own skills directory (for dev/test)
    let repo_skills = repo_root.join(SKILL_SOURCE_DIR);
    if repo_skills.exists() {
        return Ok(repo_skills);
    }

    // Fallback: check relative to the current executable (release bundle)
    let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())).unwrap_or_default();
    let bundle_skills = exe_dir.join(SKILL_SOURCE_DIR);
    if bundle_skills.exists() {
        return Ok(bundle_skills);
    }

    Err(MfError::advanced_store(
        "Skill bundle not found — ensure the mf binary includes the bundled Skill".to_string(),
        Some("reinstall mf from the official release package".to_string()),
    ))
}

fn read_manifest_version(source_dir: &Path) -> Result<String> {
    let manifest_path = source_dir.join(MANIFEST_FILE);
    if !manifest_path.exists() {
        return Ok("unknown".to_string());
    }
    let data = fs::read_to_string(&manifest_path)?;
    let manifest: serde_json::Value = serde_json::from_str(&data)?;
    Ok(manifest.get("version").and_then(|v| v.as_str()).unwrap_or("unknown").to_string())
}

/// Recursively copy a directory.
fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir(&src_path, &dst_path)?;
        } else {
            let data = fs::read(&src_path)?;
            let mut f = fs::File::create(&dst_path)?;
            f.write_all(&data)?;
            f.flush()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_dry_run_reports_plan() {
        let dir = tempfile::tempdir().unwrap();
        // Create a minimal skill source in the temp repo
        let src = dir.path().join(SKILL_SOURCE_DIR);
        fs::create_dir_all(src.join("references")).unwrap();
        fs::write(src.join("SKILL.md"), "# Test Skill\n").unwrap();
        fs::write(src.join(MANIFEST_FILE), r#"{"name":"mf-source","version":"1.0.0"}"#).unwrap();

        let result = install_skill(dir.path(), false, true).unwrap();
        assert!(result.dry_run);
        assert!(!result.installed);
        assert!(result.target.contains(SKILL_TARGET));
    }

    #[test]
    fn install_creates_target() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join(SKILL_SOURCE_DIR);
        fs::create_dir_all(src.join("references")).unwrap();
        fs::write(src.join("SKILL.md"), "# Test\n").unwrap();
        fs::write(src.join(MANIFEST_FILE), r#"{"name":"mf-source","version":"1.0.0"}"#).unwrap();

        let result = install_skill(dir.path(), false, false).unwrap();
        assert!(result.installed);
        assert!(dir.path().join(SKILL_TARGET).join("SKILL.md").exists());
    }

    #[test]
    fn reinstall_same_digest_is_noop() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join(SKILL_SOURCE_DIR);
        fs::create_dir_all(src.join("references")).unwrap();
        fs::write(src.join("SKILL.md"), "# Test\n").unwrap();
        fs::write(src.join(MANIFEST_FILE), r#"{"name":"mf-source","version":"1.0.0"}"#).unwrap();

        // First install
        let r1 = install_skill(dir.path(), false, false).unwrap();
        assert!(r1.installed);

        // Second install — should be no-op
        let r2 = install_skill(dir.path(), false, false).unwrap();
        assert!(!r2.installed);
    }
}
