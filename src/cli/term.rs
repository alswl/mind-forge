use std::path::Path;

use clap::{Args, Subcommand};
use serde::Serialize;

use crate::cli::deprecation::DeprecationContext;
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
    #[command(about = "Create a term (mf extension)")]
    New(TermNewArgs),
    #[command(about = "Lint term consistency in project docs")]
    Lint(TermLintArgs),
    #[command(about = "Learn a term correction")]
    Learn(TermLearnArgs),
    #[command(about = "Fix a term metadata (mf extension)")]
    Fix(TermFixArgs),
    #[command(about = "Show term details")]
    Show(TermShowArgs),
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct TermListArgs {
    #[arg(long)]
    pub filter: Option<String>,
    /// Look up a single term by name (deprecated: use `term show <NAME>`)
    #[arg(long)]
    pub term: Option<String>,
    #[arg(short = 'p', long)]
    pub project: Option<String>,
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
    #[arg(short = 'p', long)]
    pub project: Option<String>,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct TermLintArgs {
    pub path: Option<String>,
    #[arg(long)]
    pub fix: bool,
    #[arg(long = "dry-run")]
    pub dry_run: bool,
    #[arg(short = 'p', long)]
    pub project: Option<String>,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct TermLearnArgs {
    /// Canonical term name (mind primary)
    #[arg(long)]
    pub term: Option<String>,
    /// Variant/alias for the term (mind primary)
    #[arg(long)]
    pub alias: Option<String>,
    /// Deprecated: use --term (canonical) instead
    #[arg(long)]
    pub original: Option<String>,
    /// Deprecated: use --alias (variant) instead
    #[arg(long)]
    pub correct: Option<String>,
    #[arg(short = 'p', long)]
    pub project: Option<String>,
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
    #[arg(short = 'p', long)]
    pub project: Option<String>,
}

// ---------------------------------------------------------------------------
// Term show args
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Args, Serialize)]
pub struct TermShowArgs {
    pub name: String,
    #[arg(short = 'p', long)]
    pub project: Option<String>,
}

pub fn dispatch(
    command: TermCmd,
    repo_root: Option<&std::path::PathBuf>,
    format: Format,
    deprecation: &mut DeprecationContext,
) -> Result<CommandOutcome> {
    let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;
    let cwd = std::env::current_dir().map_err(MfError::Io)?;

    match command.command {
        None => Ok(CommandOutcome::GroupHelp("term")),
        Some(TermSubcommand::New(args)) => handle_new(args, root, &cwd, format),
        Some(TermSubcommand::List(args)) => handle_list(args, root, &cwd, format, deprecation),
        Some(TermSubcommand::Lint(args)) => handle_lint(args, root, &cwd, format),
        Some(TermSubcommand::Learn(args)) => {
            // Deprecation warning for --original/--correct
            if args.original.is_some() {
                deprecation.warn_subject("--original", "--alias <variant>");
            }
            if args.correct.is_some() {
                deprecation.warn_subject("--correct", "--term <canonical>");
            }
            handle_learn(args, root, &cwd, format)
        }
        Some(TermSubcommand::Fix(args)) => handle_fix(args, root, &cwd, format),
        Some(TermSubcommand::Show(args)) => handle_show(args, root, &cwd, format),
    }
}

// ── Handle: mf term new (US1 / T017) ─────────────────────────────────────────

fn handle_new(args: TermNewArgs, root: &Path, cwd: &Path, format: Format) -> Result<CommandOutcome> {
    let term = if let Some(ref project) = args.project {
        let project_path = svc_util::resolve_project(root, Some(project.as_str()), cwd)?;
        term_svc::new_term(&project_path, &args.term, args.definition.as_deref(), &args.alias, &args.tag)?
    } else {
        term_svc::global::new_term(root, &args.term, args.definition.as_deref(), &args.alias, &args.tag)?
    };

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
    format: Format,
    deprecation: &mut DeprecationContext,
) -> Result<CommandOutcome> {
    // D5: --term <X> redirects to term show
    if let Some(ref name) = args.term {
        deprecation.warn_by_id(crate::cli::deprecation::DeprecationId::D5, None);
        let term = if let Some(ref project) = args.project {
            let project_path = svc_util::resolve_project(root, Some(project.as_str()), cwd)?;
            term_svc::show_term(&project_path, name)?
        } else {
            term_svc::global::show_term(root, name)?
        };
        return render_term_show(&term, format);
    }

    let terms = if let Some(ref project) = args.project {
        let project_path = svc_util::resolve_project(root, Some(project.as_str()), cwd)?;
        term_svc::list_terms(&project_path, args.filter.as_deref())?
    } else {
        term_svc::global::list_terms(root, args.filter.as_deref())?
    };

    match format {
        Format::Json => {
            let data = serde_json::to_value(&terms).map_err(MfError::Json)?;
            Ok(CommandOutcome::Success(data, None))
        }
        Format::Text => {
            if terms.is_empty() {
                return Ok(CommandOutcome::Success(serde_json::Value::String("No terms found.".to_string()), None));
            }
            let mut lines = Vec::new();
            lines.push(format!(
                "{:<32} {:<60} {:<20} {:<20} {}",
                "TERM", "DEFINITION", "ALIASES", "TAGS", "CORRECTIONS"
            ));
            for t in &terms {
                let def = t.definition.as_deref().unwrap_or("-");
                let def_display = if def.len() > 60 { format!("{}…", &def[..60]) } else { def.to_string() };
                let alias_display = if t.aliases.is_empty() {
                    "0".to_string()
                } else {
                    let count = t.aliases.len();
                    let first_few: Vec<&str> = t.aliases.iter().take(3).map(|s| s.as_str()).collect();
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

fn handle_lint(args: TermLintArgs, root: &Path, cwd: &Path, format: Format) -> Result<CommandOutcome> {
    // Lint always requires project context — it scans project docs for term usage.
    let project_path = svc_util::resolve_project(root, args.project.as_deref(), cwd)?;

    let effective_fix = args.fix;
    let effective_dry_run = args.fix && args.dry_run;

    let report = if let Some(ref path) = args.path {
        // Single file lint (mind primary form)
        term_svc::lint_file(&project_path, path, effective_fix, effective_dry_run)?
    } else {
        term_svc::lint_terms(&project_path, effective_fix, effective_dry_run)?
    };

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

fn compute_lint_exit_code(report: &crate::model::term::TermLintReport, fix: bool, dry_run: bool) -> u8 {
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

fn format_lint_text(report: &crate::model::term::TermLintReport, fix: bool, dry_run: bool) -> String {
    let mut lines = Vec::new();

    if fix {
        if report.findings.is_empty() && report.failures.is_empty() {
            return "No term issues found.".to_string();
        }
        if dry_run {
            // Group by path
            let mut by_path: std::collections::BTreeMap<&str, u64> = std::collections::BTreeMap::new();
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
                let count = report.findings.iter().filter(|f| f.path.as_str() == path.as_str()).count();
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
    let unique_files: std::collections::BTreeSet<&str> = report.findings.iter().map(|f| f.path.as_str()).collect();
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

fn handle_learn(args: TermLearnArgs, root: &Path, cwd: &Path, format: Format) -> Result<CommandOutcome> {
    // Resolve primary form (--term/--alias) vs deprecated form (--original/--correct)
    let (original, correct) = if args.term.is_some() || args.alias.is_some() {
        let t = args.term.as_deref().unwrap_or("");
        let a = args.alias.as_deref().unwrap_or("");
        (a, t)
    } else {
        let o = args.original.as_deref().unwrap_or("");
        let c = args.correct.as_deref().unwrap_or("");
        (o, c)
    };

    if original.is_empty() || correct.is_empty() {
        return Err(MfError::usage(
            "requires --term <canonical> and --alias <variant> (or deprecated --original/--correct)",
            Some("use 'mf term learn --term <canonical> --alias <variant>'".to_string()),
        ));
    }

    let (term, appended) = if let Some(ref project) = args.project {
        let project_path = svc_util::resolve_project(root, Some(project.as_str()), cwd)?;
        term_svc::learn_correction(&project_path, original, correct)?
    } else {
        term_svc::global::learn_correction(root, original, correct)?
    };

    match format {
        Format::Json => {
            let data = serde_json::to_value(&term).map_err(MfError::Json)?;
            Ok(CommandOutcome::Success(data, None))
        }
        Format::Text => {
            let msg = if appended {
                format!("✓ learned correction: \"{original}\" → \"{correct}\" (term: {})", term.term)
            } else {
                format!("correction already exists: \"{original}\" → \"{correct}\" (term: {})", term.term)
            };
            Ok(CommandOutcome::Success(serde_json::Value::String(msg), None))
        }
    }
}

// ── Handle: mf term fix (US6 / T041) ─────────────────────────────────────────

fn handle_fix(args: TermFixArgs, root: &Path, cwd: &Path, format: Format) -> Result<CommandOutcome> {
    let term = if let Some(ref project) = args.project {
        let project_path = svc_util::resolve_project(root, Some(project.as_str()), cwd)?;
        term_svc::fix_term(&project_path, &args.term, args.definition.as_deref(), &args.alias, &args.tag)?
    } else {
        term_svc::global::fix_term(root, &args.term, args.definition.as_deref(), &args.alias, &args.tag)?
    };

    match format {
        Format::Json => {
            let data = serde_json::to_value(&term).map_err(MfError::Json)?;
            Ok(CommandOutcome::Success(data, None))
        }
        Format::Text => {
            let alias_count = args.alias.len();
            let tag_count = args.tag.len();
            let def_status = if args.definition.is_some() { "definition changed" } else { "definition unchanged" };
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

// ── Handle: mf term show (US3b / FR-019) ────────────────────────────────────

fn render_term_show(term: &crate::model::term::Term, format: Format) -> Result<CommandOutcome> {
    match format {
        Format::Json => {
            let data = serde_json::to_value(term).map_err(MfError::Json)?;
            Ok(CommandOutcome::Success(data, None))
        }
        Format::Text => {
            let mut lines = Vec::new();
            lines.push(format!("Term:        {}", term.term));
            if let Some(def) = &term.definition {
                lines.push(format!("Definition:  {def}"));
            }
            if !term.aliases.is_empty() {
                lines.push(format!("Aliases:     {}", term.aliases.join(", ")));
            }
            if !term.tags.is_empty() {
                lines.push(format!("Tags:        {}", term.tags.join(", ")));
            }
            if !term.corrections.is_empty() {
                lines.push("Corrections:".to_string());
                for c in &term.corrections {
                    lines.push(format!("  \"{}\" → \"{}\"", c.original, c.correct));
                }
            }
            Ok(CommandOutcome::Raw(lines.join("\n"), None))
        }
    }
}

fn handle_show(args: TermShowArgs, root: &Path, cwd: &Path, format: Format) -> Result<CommandOutcome> {
    let term = if let Some(ref project) = args.project {
        let project_path = svc_util::resolve_project(root, Some(project.as_str()), cwd)?;
        term_svc::show_term(&project_path, &args.name)?
    } else {
        term_svc::global::show_term(root, &args.name)?
    };
    render_term_show(&term, format)
}
