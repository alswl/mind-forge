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
    #[command(about = "Create a term")]
    New(TermNewArgs),
    #[command(about = "Lint term consistency in project docs")]
    Lint(TermLintArgs),
    #[command(about = "Add a term correction")]
    Add(TermLearnArgs),
    #[command(about = "Update term metadata")]
    Update(TermFixArgs),
    #[command(about = "Show term details")]
    Show(TermShowArgs),
    #[command(about = "Learn a term correction (deprecated: use `add`)", hide = true)]
    Learn(TermLearnArgs),
    #[command(about = "Fix a term metadata (deprecated: use `update`)", hide = true)]
    Fix(TermFixArgs),
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
    #[arg(long)]
    pub description: Option<String>,
    #[arg(long, value_parser = clap::value_parser!(f64))]
    pub confidence: Option<f64>,
    #[arg(long = "alias")]
    pub alias: Vec<String>,
    #[arg(long = "tag")]
    pub tag: Vec<String>,
    #[arg(long = "misrecognition")]
    pub misrecognition: Vec<String>,
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
    #[arg(long)]
    pub description: Option<String>,
    #[arg(long)]
    pub clear_description: bool,
    #[arg(long, value_parser = clap::value_parser!(f64))]
    pub confidence: Option<f64>,
    #[arg(long)]
    pub clear_confidence: bool,
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
        Some(TermSubcommand::Add(args)) => {
            warn_learn_flag_deprecations(&args, deprecation);
            handle_learn(args, root, &cwd, format)
        }
        Some(TermSubcommand::Update(args)) => handle_fix(args, root, &cwd, format),
        Some(TermSubcommand::Learn(args)) => {
            deprecation.warn_subject("term learn", "term add");
            warn_learn_flag_deprecations(&args, deprecation);
            handle_learn(args, root, &cwd, format)
        }
        Some(TermSubcommand::Fix(args)) => {
            deprecation.warn_subject("term fix", "term update");
            handle_fix(args, root, &cwd, format)
        }
        Some(TermSubcommand::Show(args)) => handle_show(args, root, &cwd, format),
    }
}

fn warn_learn_flag_deprecations(args: &TermLearnArgs, deprecation: &mut DeprecationContext) {
    if args.original.is_some() {
        deprecation.warn_subject("--original", "--alias <variant>");
    }
    if args.correct.is_some() {
        deprecation.warn_subject("--correct", "--term <canonical>");
    }
}

fn new_input(args: &TermNewArgs) -> term_svc::TermInput<'_> {
    term_svc::TermInput {
        definition: args.definition.as_deref(),
        description: args.description.as_deref(),
        confidence: args.confidence,
        aliases: &args.alias,
        tags: &args.tag,
    }
}

fn fix_update(args: &TermFixArgs) -> term_svc::TermUpdate<'_> {
    term_svc::TermUpdate {
        definition: args.definition.as_deref(),
        description: args.description.as_deref(),
        clear_description: args.clear_description,
        confidence: args.confidence,
        clear_confidence: args.clear_confidence,
        aliases: &args.alias,
        tags: &args.tag,
    }
}

// ── Handle: mf term new (US1 / T017) ─────────────────────────────────────────

fn handle_new(args: TermNewArgs, root: &Path, cwd: &Path, format: Format) -> Result<CommandOutcome> {
    let term = if let Some(ref project) = args.project {
        if !args.misrecognition.is_empty() {
            return Err(MfError::usage(
                "--misrecognition is not supported on project-scoped term files",
                Some("use global terms (without --project) for --misrecognition".to_string()),
            ));
        }
        let project_path = svc_util::resolve_project(root, Some(project.as_str()), cwd)?;
        term_svc::new_term(&project_path, &args.term, new_input(&args))?
    } else {
        term_svc::global::new_term(root, &args.term, new_input(&args), &args.misrecognition)?
    };

    match format {
        Format::Json => {
            let data = serde_json::to_value(&term).map_err(MfError::Json)?;
            Ok(CommandOutcome::Success(data, None))
        }
        Format::Text => {
            let alias_count = term.aliases.len();
            let tag_count = term.tags.len();
            let misrecog_count = term.corrections.len();
            let msg = format!(
                "✓ added term: {} ({} alias{}, {} tag{}, {} misrecognition{})",
                term.term,
                alias_count,
                if alias_count == 1 { "" } else { "es" },
                tag_count,
                if tag_count == 1 { "" } else { "s" },
                misrecog_count,
                if misrecog_count == 1 { "" } else { "s" },
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
        deprecation.warn_subject("term list --term <X>", "term show <X>");
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
    // --fix (without --dry-run) exits non-zero only on write failures.
    // All other modes (read-only and --fix --dry-run) exit non-zero when findings remain.
    let has_issue = if fix && !dry_run { !report.failures.is_empty() } else { !report.findings.is_empty() };
    u8::from(has_issue)
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
            if report.scanned_files == 0 && report.skipped_files.is_empty() && report.failures.is_empty() {
                return "No terms registered.".to_string();
            }
            if report.failures.is_empty() {
                return "No term issues found.".to_string();
            }
        }
        for f in &report.findings {
            if f.safety_reason.as_deref() == Some("ambiguous") {
                let mut seen = std::collections::BTreeSet::new();
                let unique: Vec<&str> =
                    f.candidates.iter().map(|c| c.term.as_str()).filter(|t| seen.insert(*t)).collect();
                lines.push(format!(
                    "{}:{}:{}: \"{}\" ambiguous: {}",
                    f.path,
                    f.line,
                    f.column,
                    f.original,
                    unique.join(", ")
                ));
            } else {
                let confidence_part = match f.confidence {
                    Some(c) => format!(" [confidence={c:.2}]"),
                    None => String::new(),
                };
                lines.push(format!(
                    "{}:{}:{}: \"{}\" → \"{}\" [{}]{}",
                    f.path, f.line, f.column, f.original, f.correct, f.term, confidence_part
                ));
            }
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
    // The service layer's signature is (variant, canonical) — historically named
    // (original, correct). Resolve canonical/variant from either the primary
    // (--term/--alias) or the deprecated (--original/--correct) form.
    let canonical = args.term.as_deref().or(args.correct.as_deref()).filter(|s| !s.is_empty());
    let variant = args.alias.as_deref().or(args.original.as_deref()).filter(|s| !s.is_empty());

    let (Some(canonical), Some(variant)) = (canonical, variant) else {
        return Err(MfError::usage(
            "requires --term <canonical> and --alias <variant> (or deprecated --original/--correct)",
            Some("use 'mf term learn --term <canonical> --alias <variant>'".to_string()),
        ));
    };

    let (term, appended) = if let Some(ref project) = args.project {
        let project_path = svc_util::resolve_project(root, Some(project.as_str()), cwd)?;
        term_svc::learn_correction(&project_path, variant, canonical)?
    } else {
        term_svc::global::learn_correction(root, variant, canonical)?
    };

    match format {
        Format::Json => {
            let data = serde_json::to_value(&term).map_err(MfError::Json)?;
            Ok(CommandOutcome::Success(data, None))
        }
        Format::Text => {
            let msg = if appended {
                format!("✓ learned correction: \"{variant}\" → \"{canonical}\" (term: {})", term.term)
            } else {
                format!("correction already exists: \"{variant}\" → \"{canonical}\" (term: {})", term.term)
            };
            Ok(CommandOutcome::Success(serde_json::Value::String(msg), None))
        }
    }
}

// ── Handle: mf term fix (US6 / T041) ─────────────────────────────────────────

fn handle_fix(args: TermFixArgs, root: &Path, cwd: &Path, format: Format) -> Result<CommandOutcome> {
    if args.description.is_some() && args.clear_description {
        return Err(MfError::usage("--description and --clear-description are mutually exclusive", None));
    }
    if args.confidence.is_some() && args.clear_confidence {
        return Err(MfError::usage("--confidence and --clear-confidence are mutually exclusive", None));
    }

    let update = fix_update(&args);
    let term = if let Some(ref project) = args.project {
        let project_path = svc_util::resolve_project(root, Some(project.as_str()), cwd)?;
        term_svc::fix_term(&project_path, &args.term, update)?
    } else {
        term_svc::global::fix_term(root, &args.term, update)?
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
            if let Some(desc) = &term.description {
                lines.push(format!("Description: {desc}"));
            }
            if let Some(conf) = term.confidence {
                lines.push(format!("Confidence:  {conf:.2}"));
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
