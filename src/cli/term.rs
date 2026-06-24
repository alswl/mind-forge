use clap::{Args, Subcommand};
use serde::Serialize;

use crate::cli::shared_flags::DryRunFlag;
use crate::cli::shared_flags::ForceFlag;
use crate::cli::shared_flags::LintFlags;
use crate::cli::shared_flags::NoHeadersFlag;
use crate::cli::shared_flags::NoTruncFlag;
use crate::cli::shared_flags::YesFlag;
use crate::cli::CommandCtx;
use crate::cli::CommandOutcome;
use crate::error::{MfError, Result};
use crate::model::Resource;
use crate::output::confirm::{require_confirmation, ConfirmArgs};
use crate::output::list::{json_collection, render_text, ListCell, ListOpts, ListRow, ListView};
use crate::output::show::{
    json_envelope, render_text as render_show_text, ShowBlock, ShowField, ShowOpts, ShowSection, ShowValue,
};
use crate::output::verb::{json_envelope as verb_json, render_text as verb_text, Verb, VerbOpts, VerbResult};
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
    #[command(about = "Update term metadata")]
    Update(TermUpdateArgs),
    #[command(about = "Show term details")]
    Show(TermShowArgs),
    #[command(about = "Remove a term", visible_alias = "rm")]
    Remove(TermRemoveArgs),
    #[command(about = "Rename a term")]
    Rename(TermRenameArgs),
    #[command(about = "Apply term corrections to documents (alias of `term lint --fix`)")]
    Fix(TermFixArgs),
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct TermListArgs {
    #[arg(long)]
    pub filter: Option<String>,
    #[command(flatten)]
    pub no_headers: NoHeadersFlag,
    #[command(flatten)]
    pub no_trunc: NoTruncFlag,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct TermNewArgs {
    /// Canonical term name.
    #[arg(value_name = "TERM")]
    pub term: Option<String>,
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
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct TermLintArgs {
    pub path: Option<String>,
    #[command(flatten)]
    pub lint: LintFlags,
    #[command(flatten)]
    pub yes: YesFlag,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct TermUpdateArgs {
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
    #[arg(long = "delete-alias", help = "Remove an alias from the term")]
    pub delete_alias: Vec<String>,
    #[arg(long = "delete-tag", help = "Remove a tag from the term")]
    pub delete_tag: Vec<String>,
    #[arg(long = "delete-correction", help = "Remove a correction by its original (variant) text")]
    pub delete_correction: Vec<String>,
    #[arg(long = "correction-match", help = "Set correction match kind: ORIGINAL:word|substring|pinyin")]
    pub correction_match: Vec<String>,
    #[arg(long = "correction-fix", help = "Set correction fix kind: ORIGINAL:required|suggested")]
    pub correction_fix: Vec<String>,
    #[arg(long = "correction-pinyin", help = "Set correction pinyin: ORIGINAL:<pinyin>")]
    pub correction_pinyin: Vec<String>,
    #[arg(long = "correction-boundary", help = "Set correction boundary: ORIGINAL:loose|standalone")]
    pub correction_boundary: Vec<String>,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct TermFixArgs {
    pub path: Option<String>,
    #[command(flatten)]
    pub lint: LintFlags,
    #[command(flatten)]
    pub yes: YesFlag,
}

// ---------------------------------------------------------------------------
// Term show args
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Args, Serialize)]
pub struct TermShowArgs {
    pub term: String,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct TermRemoveArgs {
    pub term: String,
    #[command(flatten)]
    pub force: ForceFlag,
    #[command(flatten)]
    pub yes: YesFlag,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct TermRenameArgs {
    pub old_term: String,
    pub new_term: String,
    #[arg(long)]
    pub keep_alias: bool,
    #[command(flatten)]
    pub force: ForceFlag,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

pub fn dispatch(command: TermCmd, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    match command.command {
        None => Ok(CommandOutcome::GroupHelp("term")),
        Some(TermSubcommand::New(args)) => handle_new(args, ctx),
        Some(TermSubcommand::List(args)) => handle_list(args, ctx),
        Some(TermSubcommand::Lint(args)) => handle_lint(args, ctx),
        Some(TermSubcommand::Update(args)) => handle_update(args, ctx),
        Some(TermSubcommand::Fix(args)) => {
            let mut lint_args = TermLintArgs { path: args.path, lint: args.lint, yes: args.yes.clone() };
            lint_args.lint.fix = true; // term fix is always lint --fix
            handle_lint(lint_args, ctx)
        }
        Some(TermSubcommand::Show(args)) => handle_show(args, ctx),
        Some(TermSubcommand::Remove(args)) => handle_remove(args, ctx),
        Some(TermSubcommand::Rename(args)) => handle_rename(args, ctx),
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

fn update_input<'a>(
    args: &'a TermUpdateArgs,
    correction_match: &'a [(String, crate::model::term::MatchKind)],
    correction_fix: &'a [(String, crate::model::term::FixKind)],
    correction_pinyin: &'a [(String, String)],
    correction_boundary: &'a [(String, crate::model::term::Boundary)],
) -> term_svc::TermUpdate<'a> {
    term_svc::TermUpdate {
        definition: args.definition.as_deref(),
        description: args.description.as_deref(),
        clear_description: args.clear_description,
        confidence: args.confidence,
        clear_confidence: args.clear_confidence,
        aliases: &args.alias,
        tags: &args.tag,
        delete_aliases: &args.delete_alias,
        delete_tags: &args.delete_tag,
        delete_corrections: &args.delete_correction,
        correction_match,
        correction_fix,
        correction_pinyin,
        correction_boundary,
    }
}

fn parse_kv(raw: &str) -> Result<(&str, &str)> {
    raw.find(':').map(|pos| (&raw[..pos], &raw[pos + 1..])).ok_or_else(|| {
        MfError::usage(
            format!("expected ORIGINAL:VALUE format, got '{raw}'"),
            Some("use --correction-<field> ORIGINAL:VALUE".to_string()),
        )
    })
}

fn parse_match_kind(raw: &str) -> Result<crate::model::term::MatchKind> {
    match raw.to_lowercase().as_str() {
        "word" => Ok(crate::model::term::MatchKind::Word),
        "substring" => Ok(crate::model::term::MatchKind::Substring),
        "pinyin" => Ok(crate::model::term::MatchKind::Pinyin),
        other => {
            Err(MfError::usage(format!("invalid match kind '{other}'; expected word, substring, or pinyin"), None))
        }
    }
}

fn parse_fix_kind(raw: &str) -> Result<crate::model::term::FixKind> {
    match raw.to_lowercase().as_str() {
        "required" => Ok(crate::model::term::FixKind::Required),
        "suggested" => Ok(crate::model::term::FixKind::Suggested),
        other => Err(MfError::usage(format!("invalid fix kind '{other}'; expected required or suggested"), None)),
    }
}

fn parse_boundary(raw: &str) -> Result<crate::model::term::Boundary> {
    match raw.to_lowercase().as_str() {
        "loose" => Ok(crate::model::term::Boundary::Loose),
        "standalone" => Ok(crate::model::term::Boundary::Standalone),
        other => Err(MfError::usage(format!("invalid boundary '{other}'; expected loose or standalone"), None)),
    }
}

// ── Handle: mf term new (US1 / T017) ─────────────────────────────────────────

fn handle_new(args: TermNewArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let warnings: Vec<String> = Vec::new();
    let term_name = args.term.clone().ok_or_else(|| {
        MfError::usage(
            "term name required",
            Some("pass the canonical term as the positional argument: `mf term new <NAME>`".to_string()),
        )
    })?;

    if args.dry_run.dry_run {
        let has_aliases = !args.alias.is_empty();
        let mut actions = vec!["create canonical term".to_string()];
        if has_aliases {
            actions.push("attach alias".to_string());
            for alias in &args.alias {
                actions.push(format!("  alias: {alias}"));
            }
        }
        let details = serde_json::json!({
            "definition": args.definition,
            "aliases": args.alias,
            "tags": args.tag,
            "planned_actions": actions,
        });
        let result = VerbResult {
            verb: Verb::Create,
            kind: "term",
            identity: term_name.clone(),
            old_identity: None,
            path: None,
            dry_run: true,
            details,
        };
        return match ctx.format() {
            Format::Json => Ok(CommandOutcome::Success(verb_json(&result), warnings, None)),
            Format::Text => {
                let mut lines: Vec<String> = Vec::new();
                lines.push(format!("[dry-run] would create term: {}", term_name));
                for alias in &args.alias {
                    lines.push(format!("[dry-run] would attach alias: {} → {}", alias, term_name));
                }
                Ok(CommandOutcome::Success(serde_json::Value::String(lines.join("\n")), warnings, None))
            }
        };
    }

    let root = ctx.require_repo_path()?;
    let result = if let Some(project_name) = ctx.project() {
        let project_path = svc_util::resolve_project(root, Some(project_name), ctx.cwd())?;
        term_svc::new_term(&project_path, &term_name, new_input(&args), &args.misrecognition)?
    } else {
        term_svc::global::new_term(root, &term_name, new_input(&args), &args.misrecognition)?
    };

    let data = serde_json::json!({
        "term": result.term.term,
        "created": result.created,
        "added_aliases": result.added_aliases,
        "added_tags": result.added_tags,
        "added_misrecognitions": result.added_misrecognitions,
    });

    let text_output = if result.created {
        if result.added_aliases.is_empty() {
            format!("created term \"{}\"", result.term.term)
        } else {
            format!("created term \"{}\" with alias {}", result.term.term, result.added_aliases.join(", "))
        }
    } else if !result.added_aliases.is_empty() {
        format!("added alias {} to existing term \"{}\"", result.added_aliases.join(", "), result.term.term)
    } else {
        format!("term \"{}\" already up to date", result.term.term)
    };

    match ctx.format() {
        Format::Json => Ok(CommandOutcome::Success(data, warnings, None)),
        Format::Text => Ok(CommandOutcome::Success(serde_json::Value::String(text_output), warnings, None)),
    }
}

// ── Handle: mf term list (US2 / T021) ────────────────────────────────────────

fn handle_list(args: TermListArgs, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    let (terms, scope_map) = if let Some(project_name) = ctx.project() {
        let project_path = svc_util::resolve_project(root, Some(project_name), ctx.cwd())?;
        let mut merged = term_svc::list_terms(&project_path, args.filter.as_deref())?;
        let global_terms = term_svc::global::list_terms(root, args.filter.as_deref())?;
        let mut scope_map: std::collections::HashMap<String, &'static str> = std::collections::HashMap::new();
        for t in &merged {
            scope_map.insert(t.term.clone(), "project");
        }
        for t in global_terms {
            if !scope_map.contains_key(&t.term) {
                scope_map.insert(t.term.clone(), "global");
                merged.push(t);
            }
        }
        merged.sort_by(|a, b| a.term.cmp(&b.term));
        (merged, scope_map)
    } else {
        let terms = term_svc::global::list_terms(root, args.filter.as_deref())?;
        (terms, std::collections::HashMap::new())
    };

    let opts = ListOpts::from_flags(args.no_headers.no_headers, args.no_trunc.no_trunc)
        .with_repo_root(Some(root.to_path_buf()));

    match ctx.format() {
        Format::Json => {
            let items: Vec<serde_json::Value> = terms
                .iter()
                .map(|t| {
                    let mut v = serde_json::to_value(t).map_err(MfError::Json)?;
                    if let Some(obj) = v.as_object_mut() {
                        obj.insert("identity".to_string(), serde_json::Value::String(t.identity()));
                        if let Some(scope) = scope_map.get(&t.term) {
                            obj.insert("scope".to_string(), serde_json::Value::String((*scope).to_string()));
                        }
                    }
                    Ok(v)
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(CommandOutcome::Success(json_collection("terms", items), Vec::new(), None))
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

fn handle_lint(args: TermLintArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    use std::io::IsTerminal;

    let root = ctx.require_repo_path()?;
    let effective_fix = args.lint.fix;
    let effective_dry_run = args.lint.fix && args.lint.dry_run;
    let include_suggested = args.lint.include_suggested;
    let warnings: Vec<String> = Vec::new();

    // US1: confirmation gate for --fix (non-dry-run)
    if effective_fix && !effective_dry_run && !args.yes.yes {
        if !std::io::stdout().is_terminal() {
            return Err(MfError::usage(
                "--fix in non-interactive context requires -y / --yes",
                Some("pass --yes to confirm".to_string()),
            ));
        }
        // Show dry-run preview in text mode before prompting
        if matches!(ctx.format(), Format::Text) {
            let preview = if let Some(pn) = ctx.project() {
                let pp = svc_util::resolve_project(root, Some(pn), ctx.cwd())?;
                if let Some(ref path) = args.path {
                    term_svc::lint_path_with_global(&pp, root, path, true, true, include_suggested)?
                } else {
                    term_svc::lint_terms_with_global(&pp, root, true, true, include_suggested)?
                }
            } else if let Some(ref path) = args.path {
                term_svc::global::lint_path(root, path, true, true, include_suggested)?
            } else {
                term_svc::global::lint_terms(root, true, true, include_suggested)?
            };
            if !preview.findings.is_empty() {
                println!("{}", format_lint_text(&preview, true, true));
            }
        }
        match crate::output::confirm::prompt_confirmation("Apply changes? [y/N] ") {
            crate::output::confirm::ConfirmOutcome::Confirmed => {}
            crate::output::confirm::ConfirmOutcome::Aborted => {
                return Ok(CommandOutcome::Raw("Aborted by user.".to_string(), Some(0)));
            }
            crate::output::confirm::ConfirmOutcome::NotTty => {
                return Err(MfError::usage(
                    "--fix in non-interactive context requires -y / --yes",
                    Some("pass --yes to confirm".to_string()),
                ));
            }
        }
    }

    let cwd = ctx.cwd();
    let report = if let Some(project_name) = ctx.project() {
        let project_path = svc_util::resolve_project(root, Some(project_name), cwd)?;
        if let Some(ref path) = args.path {
            let resolved = svc_util::path::resolve_lint_path(path, Some(&project_path), cwd, root)?;
            // Canonicalize before the containment check; otherwise `..` components
            // in `resolved` slip past `strip_prefix` and let the path escape the
            // project root.
            let canon_resolved = resolved.canonicalize().map_err(MfError::Io)?;
            let canon_project = project_path.canonicalize().map_err(MfError::Io)?;
            let rel = canon_resolved.strip_prefix(&canon_project).map_err(|_| {
                MfError::usage(
                    format!("path '{}' is not under project root '{}'", resolved.display(), project_path.display()),
                    None,
                )
            })?;
            term_svc::lint_path_with_global(
                &project_path,
                root,
                &rel.to_string_lossy(),
                effective_fix,
                effective_dry_run,
                include_suggested,
            )?
        } else {
            term_svc::lint_terms_with_global(&project_path, root, effective_fix, effective_dry_run, include_suggested)?
        }
    } else if let Some(ref path) = args.path {
        let resolved = svc_util::path::resolve_lint_path(path, None, cwd, root)?;
        term_svc::global::lint_path(
            root,
            &resolved.to_string_lossy(),
            effective_fix,
            effective_dry_run,
            include_suggested,
        )?
    } else {
        term_svc::global::lint_terms(root, effective_fix, effective_dry_run, include_suggested)?
    };

    // Determine exit code
    let base_exit = compute_lint_exit_code(&report, effective_fix, effective_dry_run);
    let warnings_count = report.findings.len() as i32;
    let exit_code =
        if args.lint.max_warnings.is_some_and(|max| warnings_count > max) { Some(1) } else { Some(base_exit) };

    match ctx.format() {
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
            Ok(CommandOutcome::Success(serde_json::Value::Object(data), warnings, exit_code))
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
                let suggested_mark = if f.fix_kind == crate::model::term::FixKind::Suggested { "?" } else { "" };
                let boundary_mark =
                    if f.boundary == crate::model::term::Boundary::Standalone { ", standalone" } else { "" };
                lines.push(format!(
                    "{}:{}:{}: \"{}\" → \"{}\" [{}]{}{}{}",
                    f.path,
                    f.line,
                    f.column,
                    f.original,
                    f.correct,
                    f.term,
                    confidence_part,
                    suggested_mark,
                    boundary_mark
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

// ── Handle: mf term update (metadata) ─────────────────────────────────────────

fn handle_update(args: TermUpdateArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    if args.description.is_some() && args.clear_description {
        return Err(MfError::usage("--description and --clear-description are mutually exclusive", None));
    }
    if args.confidence.is_some() && args.clear_confidence {
        return Err(MfError::usage("--confidence and --clear-confidence are mutually exclusive", None));
    }

    let root = ctx.require_repo_path()?;

    let mut correction_match: Vec<(String, crate::model::term::MatchKind)> = Vec::new();
    for raw in &args.correction_match {
        let (original, value) = parse_kv(raw)?;
        correction_match.push((original.to_string(), parse_match_kind(value)?));
    }
    let mut correction_fix: Vec<(String, crate::model::term::FixKind)> = Vec::new();
    for raw in &args.correction_fix {
        let (original, value) = parse_kv(raw)?;
        correction_fix.push((original.to_string(), parse_fix_kind(value)?));
    }
    let mut correction_pinyin: Vec<(String, String)> = Vec::new();
    for raw in &args.correction_pinyin {
        let (original, value) = parse_kv(raw)?;
        correction_pinyin.push((original.to_string(), value.to_string()));
    }
    let mut correction_boundary: Vec<(String, crate::model::term::Boundary)> = Vec::new();
    for raw in &args.correction_boundary {
        let (original, value) = parse_kv(raw)?;
        correction_boundary.push((original.to_string(), parse_boundary(value)?));
    }

    let update = update_input(&args, &correction_match, &correction_fix, &correction_pinyin, &correction_boundary);
    let mut warnings: Vec<String> = Vec::new();
    let (term, global_fallback) = if let Some(project_name) = ctx.project() {
        let project_path = svc_util::resolve_project(root, Some(project_name), ctx.cwd())?;
        match term_svc::fix_term(&project_path, &args.term, update) {
            Ok(term) => (term, false),
            Err(MfError::NotFound { .. }) => {
                // Try global fallback; emit the WARN only when that write actually succeeds.
                let term = term_svc::global::fix_term(root, &args.term, update)?;
                crate::output::warning::emit_warning(
                    &format!("-p {project_name} was ignored; the write applied to global scope because \"{}\" does not exist as a project-local term.", args.term),
                    &mut warnings,
                );
                (term, true)
            }
            Err(e) => return Err(e),
        }
    } else {
        (term_svc::global::fix_term(root, &args.term, update)?, false)
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
    if !args.delete_alias.is_empty() {
        changes.insert("aliases".to_string(), serde_json::json!({"deleted": args.delete_alias}));
    }
    if !args.tag.is_empty() {
        changes.insert("tags".to_string(), serde_json::json!({"added": args.tag}));
    }
    if !args.delete_tag.is_empty() {
        changes.insert("tags".to_string(), serde_json::json!({"deleted": args.delete_tag}));
    }
    if !args.delete_correction.is_empty() {
        changes.insert("corrections".to_string(), serde_json::json!({"deleted": args.delete_correction}));
    }
    if !correction_match.is_empty() {
        changes.insert("correction_match".to_string(), serde_json::json!({"updated": args.correction_match}));
    }
    if !correction_fix.is_empty() {
        changes.insert("correction_fix".to_string(), serde_json::json!({"updated": args.correction_fix}));
    }
    if !correction_pinyin.is_empty() {
        changes.insert("correction_pinyin".to_string(), serde_json::json!({"updated": args.correction_pinyin}));
    }
    if !correction_boundary.is_empty() {
        changes.insert("correction_boundary".to_string(), serde_json::json!({"updated": args.correction_boundary}));
    }

    let mut result = VerbResult {
        verb: Verb::Update,
        kind: "term",
        identity: term.term.clone(),
        old_identity: None,
        path: None,
        dry_run: false,
        details: serde_json::json!({"changes": changes}),
    };
    if global_fallback {
        if let serde_json::Value::Object(ref mut map) = result.details {
            map.insert("scope".to_string(), serde_json::json!("global"));
        }
    }
    match ctx.format() {
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), warnings, None)),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root)))),
            warnings,
            None,
        )),
    }
}

// ── Handle: mf term show (US3b / FR-019) ────────────────────────────────────

fn render_term_show(
    term: &crate::model::term::Term,
    repo_root: Option<&std::path::Path>,
    format: Format,
) -> Result<CommandOutcome> {
    render_term_show_inner(term, repo_root, format, None)
}

fn render_term_show_with_scope(
    term: &crate::model::term::Term,
    repo_root: Option<&std::path::Path>,
    scope: &str,
    format: Format,
) -> Result<CommandOutcome> {
    render_term_show_inner(term, repo_root, format, Some(scope))
}

fn render_term_show_inner(
    term: &crate::model::term::Term,
    repo_root: Option<&std::path::Path>,
    format: Format,
    scope: Option<&str>,
) -> Result<CommandOutcome> {
    let corr_count = term.corrections.len();
    let mut fields = vec![
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
    if let Some(s) = scope {
        fields.push(ShowField { label: "Scope", value: ShowValue::Text(s.to_string()) });
    }

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
            let mut extra = serde_json::to_value(term).map_err(MfError::Json)?.as_object().cloned().unwrap_or_default();
            if let Some(s) = scope {
                extra.insert("scope".to_string(), serde_json::json!(s));
            }
            Ok(CommandOutcome::Success(json_envelope(&block, extra), Vec::new(), None))
        }
        Format::Text => Ok(CommandOutcome::Raw(render_show_text(&block, &ShowOpts::from_repo_root(repo_root)), None)),
    }
}

fn handle_show(args: TermShowArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    let format = ctx.format();
    if let Some(project_name) = ctx.project() {
        let project_path = svc_util::resolve_project(root, Some(project_name), ctx.cwd())?;
        match term_svc::show_term(&project_path, &args.term) {
            Ok(term) => render_term_show_with_scope(&term, Some(root), "project", format),
            Err(MfError::NotFound { .. }) => {
                // Fall through to global
                let term = term_svc::global::show_term(root, &args.term)?;
                render_term_show_with_scope(&term, Some(root), "global", format)
            }
            Err(e) => Err(e),
        }
    } else {
        let term = term_svc::global::show_term(root, &args.term)?;
        render_term_show(&term, Some(root), format)
    }
}

// ── Handle: mf term remove / rm ────────────────────────────────────────────

fn handle_remove(args: TermRemoveArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    require_confirmation(&ConfirmArgs {
        verb_label: "removal",
        kind: "term",
        identity: &args.term,
        yes: args.yes.yes,
        force: args.force.force,
    })?;

    let root = ctx.require_repo_path()?;
    let mut warnings: Vec<String> = Vec::new();
    let report = if let Some(project_name) = ctx.project() {
        let project_path = svc_util::resolve_project(root, Some(project_name), ctx.cwd())?;
        match term_svc::remove_term(&project_path, &args.term, args.force.force, args.dry_run.dry_run) {
            Ok(report) => report,
            Err(MfError::NotFound { .. }) => {
                let report = term_svc::remove_term_global(root, &args.term, args.force.force, args.dry_run.dry_run)?;
                crate::output::warning::emit_warning(
                    &format!("-p {project_name} was ignored; the write applied to global scope because \"{}\" does not exist as a project-local term.", args.term),
                    &mut warnings,
                );
                report
            }
            Err(e) => return Err(e),
        }
    } else {
        term_svc::remove_term_global(root, &args.term, args.force.force, args.dry_run.dry_run)?
    };

    let result = VerbResult {
        verb: Verb::Remove,
        kind: "term",
        identity: report.before.name.clone(),
        old_identity: None,
        path: None,
        dry_run: args.dry_run.dry_run,
        details: serde_json::json!({"removed": true}),
    };
    match ctx.format() {
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), warnings, None)),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root)))),
            warnings,
            None,
        )),
    }
}

// ── Handle: mf term rename ──────────────────────────────────────────────────

fn handle_rename(args: TermRenameArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    if args.dry_run.dry_run {
        let result = VerbResult {
            verb: Verb::Rename,
            kind: "term",
            identity: args.new_term.clone(),
            old_identity: Some(args.old_term.clone()),
            path: None,
            dry_run: true,
            details: serde_json::json!({"keep_alias": args.keep_alias}),
        };
        return match ctx.format() {
            Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
            Format::Text => Ok(CommandOutcome::Success(
                serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root)))),
                Vec::new(),
                None,
            )),
        };
    }

    let mut rename_warnings: Vec<String> = Vec::new();
    let report = if let Some(project_name) = ctx.project() {
        let project_path = svc_util::resolve_project(root, Some(project_name), ctx.cwd())?;
        match term_svc::rename_term(
            &project_path,
            &args.old_term,
            &args.new_term,
            args.keep_alias,
            args.force.force,
            false,
        ) {
            Ok(report) => report,
            Err(MfError::NotFound { .. }) => {
                let report = term_svc::global::rename_term(
                    root,
                    &args.old_term,
                    &args.new_term,
                    args.keep_alias,
                    args.force.force,
                    false,
                )?;
                crate::output::warning::emit_warning(
                    &format!("-p {project_name} was ignored; the write applied to global scope because \"{}\" does not exist as a project-local term.", args.old_term),
                    &mut rename_warnings,
                );
                report
            }
            Err(e) => return Err(e),
        }
    } else {
        term_svc::global::rename_term(root, &args.old_term, &args.new_term, args.keep_alias, args.force.force, false)?
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
    match ctx.format() {
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), rename_warnings, None)),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root)))),
            rename_warnings,
            None,
        )),
    }
}
