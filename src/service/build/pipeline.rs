use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

use crate::error::{MfError, Result};
use crate::model::config::BuildPipelineRule;

#[derive(Debug, Clone, Serialize)]
pub struct PipelineStage {
    pub rule: String,
    pub input: String,
    pub output: String,
    pub stale: bool,
    pub action: String,
}

pub fn execute(
    project_root: &Path,
    assets_root: &Path,
    rules: &[BuildPipelineRule],
    dry_run: bool,
) -> Result<(Vec<PipelineStage>, Vec<String>)> {
    if rules.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    if !assets_root.exists() {
        return Ok((Vec::new(), Vec::new()));
    }
    let order = rule_order(rules)?;
    let mut stages = Vec::new();
    let mut collisions = BTreeMap::<PathBuf, String>::new();
    let mut failed_outputs = BTreeSet::new();
    let mut warnings = Vec::new();
    let mut virtual_files: BTreeSet<PathBuf> = files_with_extension(assets_root, "")?.into_iter().collect();

    for index in order {
        let rule = &rules[index];
        let mut inputs: Vec<PathBuf> = virtual_files
            .iter()
            .filter(|path| path.extension().is_some_and(|ext| ext == std::ffi::OsStr::new(&rule.input_extension)))
            .cloned()
            .collect();
        inputs.sort();
        for input in inputs {
            let input = canonical_asset_path(assets_root, &input)?;
            let output = input.with_extension(&rule.output_extension);
            let output = canonical_asset_path(assets_root, &output)?;
            if let Some(previous) = collisions.insert(output.clone(), rule.name.clone()) {
                return Err(MfError::usage(
                    format!(
                        "build.pipeline output collision: '{}' and '{}' produce {}",
                        previous,
                        rule.name,
                        output.display()
                    ),
                    None,
                ));
            }
            let stale = !output.exists() || modified_after(&input, &output)?;
            let blocked = failed_outputs.contains(&input);
            let action = if blocked {
                "skipped (dependency failed)"
            } else if stale {
                "run"
            } else {
                "skip (fresh)"
            };
            stages.push(PipelineStage {
                rule: rule.name.clone(),
                input: relative(project_root, &input),
                output: relative(project_root, &output),
                stale,
                action: action.to_string(),
            });
            virtual_files.insert(output.clone());
            if blocked || !stale || dry_run {
                continue;
            }
            let prior_output = output.exists().then(|| fs::read(&output)).transpose().map_err(MfError::Io)?;
            let command = expand_command(&rule.command, &input, &output, rule.dark);
            let result = Command::new("sh").args(["-c", &command]).status();
            if result.as_ref().is_err() || !result.ok().is_some_and(|status| status.success()) {
                if let Some(bytes) = prior_output {
                    fs::write(&output, bytes).map_err(MfError::Io)?;
                } else if output.exists() {
                    fs::remove_file(&output).map_err(MfError::Io)?;
                }
                failed_outputs.insert(output.clone());
                warnings.push(format!(
                    "pipeline stage '{}' failed; kept existing output {}",
                    rule.name,
                    output.display()
                ));
            }
        }
    }

    warnings.extend(
        stages
            .iter()
            .filter(|stage| stage.action == "skipped (dependency failed)")
            .map(|stage| format!("pipeline stage '{}' skipped because its input stage failed", stage.rule)),
    );
    Ok((stages, warnings))
}

fn files_with_extension(root: &Path, extension: &str) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(root).follow_links(false) {
        let entry = entry.map_err(|e| MfError::Io(std::io::Error::other(e)))?;
        if entry.file_type().is_file()
            && (extension.is_empty() || entry.path().extension().is_some_and(|ext| ext == extension))
        {
            files.push(entry.path().to_path_buf());
        }
    }
    Ok(files)
}

fn modified_after(input: &Path, output: &Path) -> Result<bool> {
    Ok(fs::metadata(input).map_err(MfError::Io)?.modified().map_err(MfError::Io)?
        > fs::metadata(output).map_err(MfError::Io)?.modified().map_err(MfError::Io)?)
}

fn canonical_asset_path(root: &Path, target: &Path) -> Result<PathBuf> {
    let root = root.canonicalize().map_err(MfError::Io)?;
    if let Ok(metadata) = fs::symlink_metadata(target)
        && metadata.file_type().is_symlink()
    {
        return Err(MfError::usage(format!("pipeline path '{}' must not be a symlink", target.display()), None));
    }
    if target.exists() {
        let path = target.canonicalize().map_err(MfError::Io)?;
        if path.starts_with(&root) {
            Ok(path)
        } else {
            Err(MfError::usage(format!("pipeline path '{}' escapes assets", target.display()), None))
        }
    } else {
        let parent = target.parent().ok_or_else(|| MfError::usage("pipeline output has no parent", None))?;
        let parent = parent.canonicalize().map_err(MfError::Io)?;
        if !parent.starts_with(&root) {
            return Err(MfError::usage(format!("pipeline path '{}' escapes assets", target.display()), None));
        }
        Ok(parent.join(target.file_name().unwrap_or_default()))
    }
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root).unwrap_or(path).to_string_lossy().replace('\\', "/")
}

fn expand_command(template: &str, input: &Path, output: &Path, dark: bool) -> String {
    template
        .replace("{input}", &shell_quote(input))
        .replace("{output}", &shell_quote(output))
        .replace("{dark}", if dark { "true" } else { "false" })
}

fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}

fn rule_order(rules: &[BuildPipelineRule]) -> Result<Vec<usize>> {
    let mut by_input = BTreeMap::new();
    let mut indegree = vec![0usize; rules.len()];
    let mut edges = vec![Vec::new(); rules.len()];
    for (index, rule) in rules.iter().enumerate() {
        by_input.insert(rule.input_extension.as_str(), index);
    }
    for (index, rule) in rules.iter().enumerate() {
        if let Some(&dependent) = by_input.get(rule.output_extension.as_str()) {
            edges[index].push(dependent);
            indegree[dependent] += 1;
        }
    }
    let mut queue: VecDeque<usize> = indegree.iter().enumerate().filter_map(|(i, d)| (*d == 0).then_some(i)).collect();
    let mut order = Vec::new();
    while let Some(index) = queue.pop_front() {
        order.push(index);
        for &next in &edges[index] {
            indegree[next] -= 1;
            if indegree[next] == 0 {
                queue.push_back(next);
            }
        }
    }
    if order.len() != rules.len() {
        return Err(MfError::usage("build.pipeline extension graph contains a cycle", None));
    }
    Ok(order)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(name: &str, input: &str, output: &str, command: &str) -> BuildPipelineRule {
        BuildPipelineRule {
            name: name.to_string(),
            input_extension: input.to_string(),
            output_extension: output.to_string(),
            command: command.to_string(),
            dark: false,
        }
    }

    #[test]
    fn plans_and_runs_stale_sibling() {
        let root = tempfile::tempdir().unwrap();
        let assets = root.path().join("assets");
        fs::create_dir_all(&assets).unwrap();
        fs::write(assets.join("diagram.svg"), "svg").unwrap();
        let rules = vec![rule("svg-to-png", "svg", "png", "cp {input} {output}")];
        let (stages, warnings) = execute(root.path(), &assets, &rules, false).unwrap();
        assert_eq!(stages[0].action, "run");
        assert!(warnings.is_empty());
        assert_eq!(fs::read_to_string(assets.join("diagram.png")).unwrap(), "svg");
    }

    #[test]
    fn orders_chained_rules_and_does_not_run_dry_plan() {
        let root = tempfile::tempdir().unwrap();
        let assets = root.path().join("assets");
        fs::create_dir_all(&assets).unwrap();
        fs::write(assets.join("diagram.d2"), "d2").unwrap();
        let rules = vec![
            rule("d2-to-svg", "d2", "svg", "cp {input} {output}"),
            rule("svg-to-png", "svg", "png", "cp {input} {output}"),
        ];
        let (stages, _) = execute(root.path(), &assets, &rules, true).unwrap();
        assert_eq!(stages.iter().map(|s| s.rule.as_str()).collect::<Vec<_>>(), vec!["d2-to-svg", "svg-to-png"]);
        assert!(!assets.join("diagram.svg").exists());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_broken_output_symlink_before_running_command() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let assets = root.path().join("assets");
        fs::create_dir_all(&assets).unwrap();
        fs::write(assets.join("diagram.svg"), "svg").unwrap();
        symlink(root.path().join("outside.png"), assets.join("diagram.png")).unwrap();
        let rules = vec![rule("svg-to-png", "svg", "png", "cp {input} {output}")];

        let error = execute(root.path(), &assets, &rules, false).unwrap_err();
        assert!(error.to_string().contains("must not be a symlink"));
        assert!(!root.path().join("outside.png").exists());
    }
}
