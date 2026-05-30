use std::path::Path;

use clap::{Args, Subcommand};
use serde::Serialize;

use crate::cli::deprecation::DeprecationContext;
use crate::cli::shared_flags::LintFlags;
use crate::cli::shared_flags::NoHeadersFlag;
use crate::cli::shared_flags::NoTruncFlag;
use crate::cli::CommandOutcome;
use crate::error::{MfError, Result};
use crate::model::Resource;
use crate::output::confirm::{require_confirmation, ConfirmArgs};
use crate::output::list::{json_collection, render_text, ListCell, ListOpts, ListRow, ListView};
use crate::output::show::{
    json_envelope, render_text as render_show_text, ShowBlock, ShowField, ShowSection, ShowValue,
};
use crate::output::verb::{json_envelope as verb_json, render_text as verb_text, Verb, VerbResult};
use crate::output::Format;
use crate::service::{term as term_svc, util as svc_util};

#[derive(Debug, Clone, Args)]
pub struct TermCmd {
    #[command(subcommand)]
    pub command: Option<TermSubcommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum TermSubcommand {
    #[command(about = "List terms", visible_alias = "ls")]
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
    #[command(about = "Remove a term", visible_alias = "rm")]
    Remove(TermRemoveArgs),
    #[command(about = "Rename a term")]
    Rename(TermRenameArgs),
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
    #[command(flatten)]
    pub no_headers: NoHeadersFlag,
    #[command(flatten)]
    pub no_trunc: NoTruncFlag,
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
    #[arg(long = "dry-run")]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct TermLintArgs {
    pub path: Option<String>,
    #[command(flatten)]
    pub lint: LintFlags,
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
    #[arg(long = "dry-run")]
    pub dry_run: bool,
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
}

// ---------------------------------------------------------------------------
// Term show args
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Args, Serialize)]
pub struct TermShowArgs {
    pub name: String,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct TermRemoveArgs {
    pub term: String,
    #[arg(short = 'f', long)]
    pub force: bool,
    #[arg(short = 'y', long)]
    pub yes: bool,
    #[arg(long = "dry-run")]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct TermRenameArgs {
    pub old_term: String,
    pub new_term: String,
    #[arg(long)]
    pub keep_alias: bool,
    #[arg(short = 'f', long)]
    pub force: bool,
    #[arg(long = "dry-run")]
    pub dry_run: bool,
}

pub fn dispatch(
    command: TermCmd,
    repo_root: Option<&std::path::PathBuf>,
    format: Format,
    project: Option<&str>,
    deprecation: &mut DeprecationContext,
) -> Result<CommandOutcome> {
    let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;
    let cwd = std::env::current_dir().map_err(MfError::Io)?;

    match command.command {
        None => Ok(CommandOutcome::GroupHelp("term")),
        Some(TermSubcommand::New(args)) => handle_new(args, root, &cwd, format, project),
        Some(TermSubcommand::List(args)) => handle_list(args, root, &cwd, format, project, deprecation),
        Some(TermSubcommand::Lint(args)) => handle_lint(args, root, &cwd, format, project),
        Some(TermSubcommand::Add(args)) => {
            warn_learn_flag_deprecations(&args, deprecation);
            handle_learn(args, root, &cwd, format, project)
        }
        Some(TermSubcommand::Update(args)) => handle_fix(args, root, &cwd, format, project),
        Some(TermSubcommand::Learn(args)) => {
            deprecation.warn_subject("term learn", "term add");
            warn_learn_flag_deprecations(&args, deprecation);
            handle_learn(args, root, &cwd, format, project)
        }
        Some(TermSubcommand::Fix(args)) => {
            deprecation.warn_subject("term fix", "term update");
            handle_fix(args, root, &cwd, format, project)
        }
        Some(TermSubcommand::Show(args)) => handle_show(args, root, &cwd, format, project),
        Some(TermSubcommand::Remove(args)) => handle_remove(args, root, &cwd, format, project),
        Some(TermSubcommand::Rename(args)) => handle_rename(args, root, &cwd, format, project),
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

fn handle_new(
    args: TermNewArgs,
    root: &Path,
    cwd: &Path,
    format: Format,
    project: Option<&str>,
) -> Result<CommandOutcome> {
    if args.dry_run {
        let result = VerbResult {
            verb: Verb::Create,
            kind: "term",
            identity: args.term.clone(),
            old_identity: None,
            path: None,
            dry_run: true,
            details: serde_json::json!({"definition": args.definition, "aliases": args.alias, "tags": args.tag}),
        };
        return match format {
            Format::Json => Ok(CommandOutcome::Success(verb_json(&result), None)),
            Format::Text => Ok(CommandOutcome::Success(serde_json::Value::String(verb_text(&result)), None)),
        };
    }

    let term = if let Some(project_name) = project {
        if !args.misrecognition.is_empty() {
            return Err(MfError::usage(
                "--misrecognition is not supported on project-scoped term files",
                Some("use global terms (without --project) for --misrecognition".to_string()),
            ));
        }
        let project_path = svc_util::resolve_project(root, Some(project_name), cwd)?;
        term_svc::new_term(&project_path, &args.term, new_input(&args))?
    } else {
        term_svc::global::new_term(root, &args.term, new_input(&args), &args.misrecognition)?
    };

    let result = VerbResult {
        verb: Verb::Create,
        kind: "term",
        identity: term.term.clone(),
        old_identity: None,
        path: None,
        dry_run: false,
        details: serde_json::to_value(&term).map_err(MfError::Json)?,
    };
    match format {
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), None)),
        Format::Text => Ok(CommandOutcome::Success(serde_json::Value::String(verb_text(&result)), None)),
    }
}

// ── Handle: mf term list (US2 / T021) ────────────────────────────────────────

fn handle_list(
    args: TermListArgs,
    root: &Path,
    cwd: &Path,
    format: Format,
    project: Option<&str>,
    deprecation: &mut DeprecationContext,
) -> Result<CommandOutcome> {
    // D5: --term <X> redirects to term show
    if let Some(ref name) = args.term {
        deprecation.warn_subject("term list --term <X>", "term show <X>");
        let term = if let Some(project_name) = project {
            let project_path = svc_util::resolve_project(root, Some(project_name), cwd)?;
            term_svc::show_term(&project_path, name)?
        } else {
            term_svc::global::show_term(root, name)?
        };
        return render_term_show(&term, format);
    }

    let terms = if let Some(project_name) = project {
        let project_path = svc_util::resolve_project(root, Some(project_name), cwd)?;
        term_svc::list_terms(&project_path, args.filter.as_deref())?
    } else {
        term_svc::global::list_terms(root, args.filter.as_deref())?
    };

    let opts = ListOpts::from_flags(args.no_headers.no_headers, args.no_trunc.no_trunc);

    match format {
        Format::Json => {
            let items: Vec<serde_json::Value> = terms
                .iter()
                .map(|t| {
                    let mut v = serde_json::to_value(t).map_err(MfError::Json)?;
                    if let Some(obj) = v.as_object_mut() {
                        obj.insert("identity".to_string(), serde_json::Value::String(t.identity()));
                    }
                    Ok(v)
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(CommandOutcome::Success(json_collection("terms", items), None))
        }
        Format::Text => {
            let mut rows = Vec::with_capacity(terms.len());
            for t in &terms {
                let def = t.definition.as_deref().unwrap_or("-").to_string();
                let alias_display = if t.aliases.is_empty() { "-".to_string() } else { t.aliases.join(", ") };
                let tags_display = if t.tags.is_empty() { "-".to_string() } else { t.tags.join(", ") };
                let corr_display =
                    if t.corrections.is_empty() { "-".to_string() } else { t.corrections.len().to_string() };
                rows.push(ListRow {
                    cells: vec![
                        ListCell::Text(t.term.clone()),
                        ListCell::Text(def),
                        ListCell::Text(alias_display),
                        ListCell::Text(tags_display),
                        ListCell::Text(corr_display),
                    ],
                });
            }
            let view = ListView {
                headers: &["TERM", "DEFINITION", "ALIASES", "TAGS", "CORRECTIONS"],
                rows,
                plural_noun: "terms",
            };
            Ok(CommandOutcome::Raw(render_text(&view, &opts), None))
        }
    }
}

// ── Handle: mf term lint (US3 / T027 + US4 / T033) ──────────────────────────

fn handle_lint(
    args: TermLintArgs,
    root: &Path,
    cwd: &Path,
    format: Format,
    project: Option<&str>,
) -> Result<CommandOutcome> {
    let effective_fix = args.lint.fix;
    let effective_dry_run = args.lint.fix && args.lint.dry_run;

    let report = if let Some(project_name) = project {
        let project_path = svc_util::resolve_project(root, Some(project_name), cwd)?;
        if let Some(ref path) = args.path {
            term_svc::lint_path(&project_path, path, effective_fix, effective_dry_run)?
        } else {
            term_svc::lint_terms(&project_path, effective_fix, effective_dry_run)?
        }
    } else if let Some(ref path) = args.path {
        term_svc::global::lint_path(root, path, effective_fix, effective_dry_run)?
    } else {
        term_svc::global::lint_terms(root, effective_fix, effective_dry_run)?
    };

    // Determine exit code
    let base_exit = compute_lint_exit_code(&report, effective_fix, effective_dry_run);
    let warnings_count = report.findings.len() as i32;
    let exit_code =
        if args.lint.max_warnings.is_some_and(|max| warnings_count > max) { Some(1) } else { Some(base_exit) };

    match format {
        Format::Json => {
            let report_value = serde_json::to_value(&report).map_err(MfError::Json)?;
            let mut data = serde_json::Map::new();
            data.insert("kind".to_string(), serde_json::Value::String("term".to_string()));
            data.insert("dry_run".to_string(), serde_json::Value::Bool(effective_dry_run));
            // Flatten report fields into data
            if let serde_json::Value::Object(obj) = report_value {
                for (k, v) in obj {
                    if k != "kind" && k != "dry_run" {
                        data.insert(k, v);
                    }
                }
            }
            Ok(CommandOutcome::Success(serde_json::Value::Object(data), exit_code))
        }
        Format::Text => {
            let output = format_lint_text(&report, effective_fix, effective_dry_run);
            Ok(CommandOutcome::Raw(output, exit_code))
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

fn handle_learn(
    args: TermLearnArgs,
    root: &Path,
    cwd: &Path,
    format: Format,
    project: Option<&str>,
) -> Result<CommandOutcome> {
    // The service layer's signature is (variant, canonical) — historically named
    // (original, correct). Resolve canonical/variant from either the primary
    // (--term/--alias) or the deprecated (--original/--correct) form.
    let canonical = args.term.as_deref().or(args.correct.as_deref()).filter(|s| !s.is_empty());
    let variant = args.alias.as_deref().or(args.original.as_deref()).filter(|s| !s.is_empty());

    let (Some(canonical), Some(variant)) = (canonical, variant) else {
        return Err(MfError::usage(
            "requires --term <canonical> and --alias <variant> (or deprecated --original/--correct)",
            Some("use `mf term learn --term <canonical> --alias <variant>`".to_string()),
        ));
    };

    if args.dry_run {
        let result = VerbResult {
            verb: Verb::Add,
            kind: "term_correction",
            identity: format!("{canonical}::{variant}"),
            old_identity: None,
            path: None,
            dry_run: true,
            details: serde_json::json!({"canonical": canonical, "variant": variant}),
        };
        return match format {
            Format::Json => Ok(CommandOutcome::Success(verb_json(&result), None)),
            Format::Text => Ok(CommandOutcome::Success(serde_json::Value::String(verb_text(&result)), None)),
        };
    }

    let (term, _appended) = if let Some(project_name) = project {
        let project_path = svc_util::resolve_project(root, Some(project_name), cwd)?;
        term_svc::learn_correction(&project_path, variant, canonical)?
    } else {
        term_svc::global::learn_correction(root, variant, canonical)?
    };

    let result = VerbResult {
        verb: Verb::Add,
        kind: "term_correction",
        identity: format!("{}::{}", term.term, variant),
        old_identity: None,
        path: None,
        dry_run: false,
        details: serde_json::to_value(&term).map_err(MfError::Json)?,
    };
    match format {
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), None)),
        Format::Text => Ok(CommandOutcome::Success(serde_json::Value::String(verb_text(&result)), None)),
    }
}

// ── Handle: mf term fix (US6 / T041) ─────────────────────────────────────────

fn handle_fix(
    args: TermFixArgs,
    root: &Path,
    cwd: &Path,
    format: Format,
    project: Option<&str>,
) -> Result<CommandOutcome> {
    if args.description.is_some() && args.clear_description {
        return Err(MfError::usage("--description and --clear-description are mutually exclusive", None));
    }
    if args.confidence.is_some() && args.clear_confidence {
        return Err(MfError::usage("--confidence and --clear-confidence are mutually exclusive", None));
    }

    let update = fix_update(&args);
    let term = if let Some(project_name) = project {
        let project_path = svc_util::resolve_project(root, Some(project_name), cwd)?;
        term_svc::fix_term(&project_path, &args.term, update)?
    } else {
        term_svc::global::fix_term(root, &args.term, update)?
    };

    let mut changes = serde_json::Map::new();
    if args.definition.is_some() {
        changes.insert("definition".to_string(), serde_json::json!({"changed": true}));
    }
    if args.description.is_some() || args.clear_description {
        changes.insert("description".to_string(), serde_json::json!({"changed": true}));
    }
    if args.confidence.is_some() || args.clear_confidence {
        changes.insert("confidence".to_string(), serde_json::json!({"changed": true}));
    }
    if !args.alias.is_empty() {
        changes.insert("aliases".to_string(), serde_json::json!({"added": args.alias}));
    }
    if !args.tag.is_empty() {
        changes.insert("tags".to_string(), serde_json::json!({"added": args.tag}));
    }

    let result = VerbResult {
        verb: Verb::Update,
        kind: "term",
        identity: term.term.clone(),
        old_identity: None,
        path: None,
        dry_run: false,
        details: serde_json::json!({"changes": changes}),
    };
    match format {
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), None)),
        Format::Text => Ok(CommandOutcome::Success(serde_json::Value::String(verb_text(&result)), None)),
    }
}

// ── Handle: mf term show (US3b / FR-019) ────────────────────────────────────

fn render_term_show(term: &crate::model::term::Term, format: Format) -> Result<CommandOutcome> {
    let corr_count = term.corrections.len();
    let fields = vec![
        ShowField { label: "Term", value: ShowValue::Text(term.term.clone()) },
        ShowField { label: "Definition", value: ShowValue::Optional(term.definition.clone()) },
        ShowField { label: "Description", value: ShowValue::Optional(term.description.clone()) },
        ShowField {
            label: "Aliases",
            value: if term.aliases.is_empty() {
                ShowValue::Text("-".to_string())
            } else {
                ShowValue::Text(term.aliases.join(", "))
            },
        },
        ShowField {
            label: "Tags",
            value: if term.tags.is_empty() {
                ShowValue::Text("-".to_string())
            } else {
                ShowValue::Text(term.tags.join(", "))
            },
        },
        ShowField { label: "Confidence", value: ShowValue::Optional(term.confidence.map(|c| format!("{c:.2}"))) },
        ShowField {
            label: "Corrections",
            value: if corr_count == 0 {
                ShowValue::Text("-".to_string())
            } else {
                ShowValue::Text(corr_count.to_string())
            },
        },
    ];

    let mut sections = Vec::new();
    if !term.corrections.is_empty() {
        let corr_fields: Vec<ShowField> = term
            .corrections
            .iter()
            .map(|c| ShowField {
                label: "Correction",
                value: ShowValue::Text(format!("\"{}\" → \"{}\"", c.original, c.correct)),
            })
            .collect();
        sections.push(ShowSection { heading: "Corrections", fields: corr_fields });
    }

    let block = ShowBlock { kind: "term", identity: term.term.clone(), fields, sections };

    match format {
        Format::Json => {
            let term_json = serde_json::to_value(term).map_err(MfError::Json)?;
            let extra = term_json.as_object().cloned().unwrap_or_default();
            Ok(CommandOutcome::Success(json_envelope(&block, extra), None))
        }
        Format::Text => Ok(CommandOutcome::Raw(render_show_text(&block), None)),
    }
}

fn handle_show(
    args: TermShowArgs,
    root: &Path,
    cwd: &Path,
    format: Format,
    project: Option<&str>,
) -> Result<CommandOutcome> {
    let term = if let Some(project_name) = project {
        let project_path = svc_util::resolve_project(root, Some(project_name), cwd)?;
        term_svc::show_term(&project_path, &args.name)?
    } else {
        term_svc::global::show_term(root, &args.name)?
    };
    render_term_show(&term, format)
}

// ── Handle: mf term remove / rm ────────────────────────────────────────────

fn handle_remove(
    args: TermRemoveArgs,
    root: &Path,
    cwd: &Path,
    format: Format,
    project: Option<&str>,
) -> Result<CommandOutcome> {
    require_confirmation(&ConfirmArgs {
        verb_label: "removal",
        kind: "term",
        identity: &args.term,
        yes: args.yes,
        force: args.force,
    })?;

    let report = if let Some(project_name) = project {
        let project_path = svc_util::resolve_project(root, Some(project_name), cwd)?;
        term_svc::remove_term(&project_path, &args.term, args.force, args.dry_run)?
    } else {
        term_svc::remove_term_global(root, &args.term, args.force, args.dry_run)?
    };

    let result = VerbResult {
        verb: Verb::Remove,
        kind: "term",
        identity: report.before.name.clone(),
        old_identity: None,
        path: None,
        dry_run: args.dry_run,
        details: serde_json::json!({"removed": true}),
    };
    match format {
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), None)),
        Format::Text => Ok(CommandOutcome::Success(serde_json::Value::String(verb_text(&result)), None)),
    }
}

// ── Handle: mf term rename ──────────────────────────────────────────────────

fn handle_rename(
    args: TermRenameArgs,
    root: &Path,
    cwd: &Path,
    format: Format,
    project: Option<&str>,
) -> Result<CommandOutcome> {
    if args.dry_run {
        let result = VerbResult {
            verb: Verb::Rename,
            kind: "term",
            identity: args.new_term.clone(),
            old_identity: Some(args.old_term.clone()),
            path: None,
            dry_run: true,
            details: serde_json::json!({"keep_alias": args.keep_alias}),
        };
        return match format {
            Format::Json => Ok(CommandOutcome::Success(verb_json(&result), None)),
            Format::Text => Ok(CommandOutcome::Success(serde_json::Value::String(verb_text(&result)), None)),
        };
    }

    let report = if let Some(project_name) = project {
        let project_path = svc_util::resolve_project(root, Some(project_name), cwd)?;
        term_svc::rename_term(&project_path, &args.old_term, &args.new_term, args.keep_alias, args.force, false)?
    } else {
        term_svc::global::rename_term(root, &args.old_term, &args.new_term, args.keep_alias, args.force, false)?
    };

    let result = VerbResult {
        verb: Verb::Rename,
        kind: "term",
        identity: report.after.name.clone(),
        old_identity: Some(report.before.name.clone()),
        path: None,
        dry_run: false,
        details: serde_json::json!({"keep_alias": args.keep_alias}),
    };
    match format {
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), None)),
        Format::Text => Ok(CommandOutcome::Success(serde_json::Value::String(verb_text(&result)), None)),
    }
}
