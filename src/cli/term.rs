use std::path::Path;

use clap::{Args, Subcommand};
use serde::Serialize;

use crate::cli::CommandOutcome;
use crate::error::{MfError, Result};
use crate::output::Format;
use crate::service::{term as term_svc, util as svc_util};

#[derive(Debug, Clone, Args)]
pub struct TermCmd {
    #[command(subcommand)]
    pub command: Option<TermSubcommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum TermSubcommand {
    #[command(about = "List terms")]
    List(TermListArgs),
    #[command(about = "Create a term")]
    New(TermNewArgs),
    #[command(about = "Lint term consistency in project docs")]
    Lint(TermLintArgs),
    #[command(about = "Learn a term correction")]
    Learn(TermLearnArgs),
    #[command(about = "Fix a term metadata")]
    Fix(TermFixArgs),
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct TermListArgs {
    #[arg(long)]
    pub filter: Option<String>,
    #[arg(long)]
    pub project: Option<String>,
    #[arg(long, value_enum, default_value_t = Format::Text)]
    pub format: Format,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct TermNewArgs {
    pub term: String,
    #[arg(long)]
    pub definition: Option<String>,
    #[arg(long = "alias")]
    pub alias: Vec<String>,
    #[arg(long = "tag")]
    pub tag: Vec<String>,
    #[arg(long)]
    pub project: Option<String>,
    #[arg(long, value_enum, default_value_t = Format::Text)]
    pub format: Format,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct TermLintArgs {
    #[arg(long)]
    pub fix: bool,
    #[arg(long = "dry-run")]
    pub dry_run: bool,
    #[arg(long)]
    pub project: Option<String>,
    #[arg(long, value_enum, default_value_t = Format::Text)]
    pub format: Format,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct TermLearnArgs {
    #[arg(long)]
    pub original: String,
    #[arg(long)]
    pub correct: String,
    #[arg(long)]
    pub project: Option<String>,
    #[arg(long, value_enum, default_value_t = Format::Text)]
    pub format: Format,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct TermFixArgs {
    pub term: String,
    #[arg(long)]
    pub definition: Option<String>,
    #[arg(long = "alias")]
    pub alias: Vec<String>,
    #[arg(long = "tag")]
    pub tag: Vec<String>,
    #[arg(long)]
    pub project: Option<String>,
    #[arg(long, value_enum, default_value_t = Format::Text)]
    pub format: Format,
}

pub fn dispatch(
    command: TermCmd,
    repo_root: Option<&std::path::PathBuf>,
    global_format: Format,
) -> Result<CommandOutcome> {
    let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;
    let cwd = std::env::current_dir().map_err(MfError::Io)?;

    match command.command {
        None => Ok(CommandOutcome::GroupHelp(super::HelpTarget::Term)),
        Some(TermSubcommand::New(args)) => handle_new(args, root, &cwd, global_format),
        Some(TermSubcommand::List(args)) => handle_list(args, root, &cwd, global_format),
        Some(TermSubcommand::Lint(args)) => handle_lint(args, root, &cwd, global_format),
        Some(TermSubcommand::Learn(args)) => handle_learn(args, root, &cwd, global_format),
        Some(TermSubcommand::Fix(args)) => handle_fix(args, root, &cwd, global_format),
    }
}

// ── Handle: mf term new (US1 / T017) ─────────────────────────────────────────

fn handle_new(
    args: TermNewArgs,
    root: &Path,
    cwd: &Path,
    global_format: Format,
) -> Result<CommandOutcome> {
    let format = if args.format == Format::Text { global_format } else { args.format };
    let project_path = svc_util::resolve_project(root, args.project.as_deref(), cwd)?;
    let term = term_svc::new_term(
        &project_path,
        &args.term,
        args.definition.as_deref(),
        &args.alias,
        &args.tag,
    )?;

    match format {
        Format::Json => {
            let data = serde_json::to_value(&term).map_err(MfError::Json)?;
            Ok(CommandOutcome::Success(data, None))
        }
        Format::Text => {
            let alias_count = term.aliases.len();
            let tag_count = term.tags.len();
            let msg = format!(
                "✓ added term: {} ({} alias{}, {} tag{})",
                term.term,
                alias_count,
                if alias_count == 1 { "" } else { "es" },
                tag_count,
                if tag_count == 1 { "" } else { "s" },
            );
            Ok(CommandOutcome::Success(serde_json::Value::String(msg), None))
        }
    }
}

// ── Handle: mf term list (US2 / T021) ────────────────────────────────────────

fn handle_list(
    args: TermListArgs,
    root: &Path,
    cwd: &Path,
    global_format: Format,
) -> Result<CommandOutcome> {
    let format = if args.format == Format::Text { global_format } else { args.format };
    let project_path = svc_util::resolve_project(root, args.project.as_deref(), cwd)?;
    let terms = term_svc::list_terms(&project_path, args.filter.as_deref())?;

    match format {
        Format::Json => {
            let data = serde_json::to_value(&terms).map_err(MfError::Json)?;
            Ok(CommandOutcome::Success(data, None))
        }
        Format::Text => {
            if terms.is_empty() {
                return Ok(CommandOutcome::Success(
                    serde_json::Value::String("No terms found.".to_string()),
                    None,
                ));
            }
            let mut lines = Vec::new();
            lines.push(format!(
                "{:<32} {:<60} {:<20} {:<20} {}",
                "TERM", "DEFINITION", "ALIASES", "TAGS", "CORRECTIONS"
            ));
            for t in &terms {
                let def = t.definition.as_deref().unwrap_or("-");
                let def_display =
                    if def.len() > 60 { format!("{}…", &def[..60]) } else { def.to_string() };
                let alias_display = if t.aliases.is_empty() {
                    "0".to_string()
                } else {
                    let count = t.aliases.len();
                    let first_few: Vec<&str> =
                        t.aliases.iter().take(3).map(|s| s.as_str()).collect();
                    let joined = first_few.join(", ");
                    if count > 3 {
                        format!("{count} ({joined}…)")
                    } else {
                        format!("{count} ({joined})")
                    }
                };
                let tags_display = t.tags.join(", ");
                let corr_count = t.corrections.len();
                lines.push(format!(
                    "{:<32} {:<60} {:<20} {:<20} {}",
                    t.term, def_display, alias_display, tags_display, corr_count
                ));
            }
            Ok(CommandOutcome::Raw(lines.join("\n"), None))
        }
    }
}

// ── Handle: mf term lint (US3 / T027 + US4 / T033) ──────────────────────────

fn handle_lint(
    args: TermLintArgs,
    root: &Path,
    cwd: &Path,
    global_format: Format,
) -> Result<CommandOutcome> {
    let format = if args.format == Format::Text { global_format } else { args.format };
    let project_path = svc_util::resolve_project(root, args.project.as_deref(), cwd)?;

    let effective_fix = args.fix;
    let effective_dry_run = args.fix && args.dry_run;

    let report = term_svc::lint_terms(&project_path, effective_fix, effective_dry_run)?;

    // Determine exit code per contracts/term-lint.md §Exit Codes
    let exit_code = compute_lint_exit_code(&report, effective_fix, effective_dry_run);

    match format {
        Format::Json => {
            let data = serde_json::to_value(&report).map_err(MfError::Json)?;
            Ok(CommandOutcome::Success(data, Some(exit_code)))
        }
        Format::Text => {
            let output = format_lint_text(&report, effective_fix, effective_dry_run);
            Ok(CommandOutcome::Raw(output, Some(exit_code)))
        }
    }
}

fn compute_lint_exit_code(
    report: &crate::model::term::TermLintReport,
    fix: bool,
    dry_run: bool,
) -> u8 {
    if fix && dry_run {
        // dry-run: findings ≥1 → exit 1
        if report.findings.is_empty() {
            0
        } else {
            1
        }
    } else if fix {
        // --fix: failures ≥1 → exit 1
        if report.failures.is_empty() {
            0
        } else {
            1
        }
    } else {
        // read-only: findings ≥1 → exit 1
        if report.findings.is_empty() {
            0
        } else {
            1
        }
    }
}

fn format_lint_text(
    report: &crate::model::term::TermLintReport,
    fix: bool,
    dry_run: bool,
) -> String {
    let mut lines = Vec::new();

    if fix {
        if report.findings.is_empty() && report.failures.is_empty() {
            return "No term issues found.".to_string();
        }
        if dry_run {
            // Group by path
            let mut by_path: std::collections::BTreeMap<&str, u64> =
                std::collections::BTreeMap::new();
            for f in &report.findings {
                *by_path.entry(f.path.as_str()).or_default() += 1;
            }
            for (path, count) in &by_path {
                let s = if *count == 1 { "" } else { "s" };
                lines.push(format!("[dry-run] would fix: {path} ({count} replacement{s})"));
            }
        } else {
            // Group by path for modified files
            for path in &report.modified_files {
                let count =
                    report.findings.iter().filter(|f| f.path.as_str() == path.as_str()).count();
                let s = if count == 1 { "" } else { "s" };
                lines.push(format!("✓ fixed: {path} ({count} replacement{s})"));
            }
            for f in &report.failures {
                lines.push(format!("✗ failed: {} — {}", f.path, f.reason));
            }
        }
    } else {
        if report.findings.is_empty() {
            if report.scanned_files == 0
                && report.findings.is_empty()
                && report.skipped_files.is_empty()
                && report.failures.is_empty()
            {
                return "No terms registered.".to_string();
            }
            if report.failures.is_empty() {
                return "No term issues found.".to_string();
            }
        }
        for f in &report.findings {
            lines.push(format!(
                "{}:{}:{}: \"{}\" → \"{}\" [{}]",
                f.path, f.line, f.column, f.original, f.correct, f.term
            ));
        }
    }

    // Summary line
    let total_findings = report.findings.len();
    let unique_files: std::collections::BTreeSet<&str> =
        report.findings.iter().map(|f| f.path.as_str()).collect();
    let file_count = unique_files.len();

    if fix && dry_run {
        let wf = report.would_fix_count.unwrap_or(0);
        lines.push(format!(
            "{total_findings} findings in {file_count} files (would fix {wf}, {} failure{})",
            report.failures.len(),
            if report.failures.len() == 1 { "" } else { "s" },
        ));
    } else if fix {
        lines.push(format!(
            "{total_findings} findings in {file_count} files ({} fixed, {} failure{})",
            report.fixed_count,
            report.failures.len(),
            if report.failures.len() == 1 { "" } else { "s" },
        ));
    } else {
        lines.push(format!("{total_findings} findings in {file_count} files"));
    }

    lines.join("\n")
}

// ── Handle: mf term learn (US5 / T037) ───────────────────────────────────────

fn handle_learn(
    args: TermLearnArgs,
    root: &Path,
    cwd: &Path,
    global_format: Format,
) -> Result<CommandOutcome> {
    let format = if args.format == Format::Text { global_format } else { args.format };
    let project_path = svc_util::resolve_project(root, args.project.as_deref(), cwd)?;

    let (term, appended) =
        term_svc::learn_correction(&project_path, &args.original, &args.correct)?;

    match format {
        Format::Json => {
            let data = serde_json::to_value(&term).map_err(MfError::Json)?;
            Ok(CommandOutcome::Success(data, None))
        }
        Format::Text => {
            let msg = if appended {
                format!(
                    "✓ learned correction: \"{}\" → \"{}\" (term: {})",
                    args.original, args.correct, term.term
                )
            } else {
                format!(
                    "correction already exists: \"{}\" → \"{}\" (term: {})",
                    args.original, args.correct, term.term
                )
            };
            Ok(CommandOutcome::Success(serde_json::Value::String(msg), None))
        }
    }
}

// ── Handle: mf term fix (US6 / T041) ─────────────────────────────────────────

fn handle_fix(
    args: TermFixArgs,
    root: &Path,
    cwd: &Path,
    global_format: Format,
) -> Result<CommandOutcome> {
    let format = if args.format == Format::Text { global_format } else { args.format };
    let project_path = svc_util::resolve_project(root, args.project.as_deref(), cwd)?;

    let term = term_svc::fix_term(
        &project_path,
        &args.term,
        args.definition.as_deref(),
        &args.alias,
        &args.tag,
    )?;

    match format {
        Format::Json => {
            let data = serde_json::to_value(&term).map_err(MfError::Json)?;
            Ok(CommandOutcome::Success(data, None))
        }
        Format::Text => {
            let alias_count = args.alias.len();
            let tag_count = args.tag.len();
            let def_status = if args.definition.is_some() {
                "definition changed"
            } else {
                "definition unchanged"
            };
            let msg = format!(
                "✓ updated term: {} ({}, +{} alias{}, +{} tag{})",
                term.term,
                def_status,
                alias_count,
                if alias_count == 1 { "" } else { "es" },
                tag_count,
                if tag_count == 1 { "" } else { "s" },
            );
            Ok(CommandOutcome::Success(serde_json::Value::String(msg), None))
        }
    }
}
