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
use crate::model::term::{Boundary, FixKind, MatchKind};
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
    #[command(about = "Manage term corrections")]
    Correction(TermCorrectionCmd),
    #[command(about = "Move a term between scopes", visible_alias = "mv")]
    Move(TermMoveArgs),
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct TermListArgs {
    /// Match the canonical term name (substring).
    #[arg(long)]
    pub filter: Option<String>,
    /// Match the term's tag field.
    #[arg(long = "tag")]
    pub tag: Vec<String>,
    /// Match the term's alias field (does not match the term name).
    #[arg(long = "alias")]
    pub alias: Vec<String>,
    /// Only show terms that have at least one correction
    #[arg(long = "has-correction")]
    pub has_correction: bool,
    /// Filter by scope: project, global, or all (default: project + global fallback)
    #[arg(long = "scope")]
    pub scope: Option<String>,
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
    /// Target a specific article by slug or title
    #[arg(long = "article")]
    pub article: Option<String>,
    /// Only apply/scan corrections for the named term(s) (repeatable, exact name)
    #[arg(long = "term", value_name = "NAME")]
    pub term: Vec<String>,
    #[command(flatten)]
    pub lint: LintFlags,
    #[command(flatten)]
    pub yes: YesFlag,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct TermFixArgs {
    pub path: Option<String>,
    /// Target a specific article by slug or title
    #[arg(long = "article")]
    pub article: Option<String>,
    /// Only apply/scan corrections for the named term(s) (repeatable, exact name)
    #[arg(long = "term", value_name = "NAME")]
    pub term: Vec<String>,
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
    /// Append a correction to the term (repeatable). Defaults to word/match, required/fix.
    #[arg(long = "add-correction", help = "Append a correction (ORIGINAL); defaults to word/required")]
    pub add_correction: Vec<String>,
    /// Set match kind of an existing correction: ORIGINAL:word|substring|pinyin (repeatable).
    #[arg(
        long = "correction-match",
        value_name = "ORIGINAL:KIND",
        help = "Set match kind of a correction: ORIGINAL:word|substring|pinyin"
    )]
    pub correction_match: Vec<String>,
    /// Set fix kind of an existing correction: ORIGINAL:required|suggested (repeatable).
    #[arg(
        long = "correction-fix",
        value_name = "ORIGINAL:KIND",
        help = "Set fix kind of a correction: ORIGINAL:required|suggested"
    )]
    pub correction_fix: Vec<String>,
    /// Set pinyin of an existing correction: ORIGINAL:PINYIN (repeatable).
    #[arg(
        long = "correction-pinyin",
        value_name = "ORIGINAL:PINYIN",
        help = "Set pinyin of a correction: ORIGINAL:PINYIN"
    )]
    pub correction_pinyin: Vec<String>,
    /// Delete a correction by original text (repeatable).
    #[arg(long = "delete-correction", help = "Delete a correction by ORIGINAL")]
    pub delete_correction: Vec<String>,
    /// Misrecognition corrections are not supported on `term update`.
    /// Use `mf term correction add` or `mf term new --misrecognition` instead.
    #[arg(long = "misrecognition", hide = true)]
    pub misrecognition: Vec<String>,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
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

// ── Correction subcommand types ──────────────────────────────────────────

#[derive(Debug, Clone, Args)]
pub struct TermCorrectionCmd {
    #[command(subcommand)]
    pub command: TermCorrectionSubcommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum TermCorrectionSubcommand {
    #[command(about = "Add a correction to a term")]
    Add(TermCorrectionAddArgs),
    #[command(about = "List corrections for a term")]
    List(TermCorrectionListArgs),
    #[command(about = "Show a single correction")]
    Show(TermCorrectionShowArgs),
    #[command(about = "Update correction attributes")]
    Update(TermCorrectionUpdateArgs),
    #[command(about = "Remove a correction from a term")]
    Remove(TermCorrectionRemoveArgs),
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct TermCorrectionAddArgs {
    pub term: String,
    pub original: String,
    pub correct: String,
    #[arg(long)]
    pub r#match: Option<String>,
    #[arg(long)]
    pub fix: Option<String>,
    #[arg(long)]
    pub pinyin: Option<String>,
    #[arg(long)]
    pub boundary: Option<String>,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct TermCorrectionListArgs {
    pub term: String,
    #[command(flatten)]
    pub no_headers: NoHeadersFlag,
    #[command(flatten)]
    pub no_trunc: NoTruncFlag,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct TermCorrectionShowArgs {
    pub term: String,
    pub original: String,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct TermCorrectionUpdateArgs {
    pub term: String,
    pub original: String,
    #[arg(long)]
    pub correct: Option<String>,
    #[arg(long)]
    pub r#match: Option<String>,
    #[arg(long)]
    pub fix: Option<String>,
    #[arg(long)]
    pub pinyin: Option<String>,
    #[arg(long)]
    pub boundary: Option<String>,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct TermCorrectionRemoveArgs {
    pub term: String,
    pub original: String,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct TermMoveArgs {
    pub term: String,
    /// Destination project for the term
    #[arg(long = "to-project")]
    pub to_project: Option<String>,
    /// Move to global scope
    #[arg(long = "to-global")]
    pub to_global: bool,
    /// Source is global scope (default: project-scoped via -p)
    #[arg(long = "from-global")]
    pub from_global: bool,
    #[command(flatten)]
    pub force: ForceFlag,
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

// ── Shared scope-warning plumbing ──────────────────────────────────────────

/// Emit a WARN when a project-scoped write fell through to global scope.
/// Reused by update, remove, and rename handlers.
fn emit_scope_fallback_warning(project_name: &str, term_name: &str, warnings: &mut Vec<String>) {
    crate::output::warning::emit_warning(
        &format!(
            "-p {project_name} was ignored; the write applied to global scope because \"{term_name}\" does not exist as a project-local term."
        ),
        warnings,
    );
}

pub fn dispatch(command: TermCmd, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    match command.command {
        None => Ok(CommandOutcome::GroupHelp("term")),
        Some(TermSubcommand::New(args)) => handle_new(args, ctx),
        Some(TermSubcommand::List(args)) => handle_list(args, ctx),
        Some(TermSubcommand::Lint(args)) => handle_lint(args, ctx),
        Some(TermSubcommand::Update(args)) => handle_update(args, ctx),
        Some(TermSubcommand::Fix(args)) => {
            let mut lint_args = TermLintArgs {
                path: args.path,
                article: args.article,
                term: args.term,
                lint: args.lint,
                yes: args.yes.clone(),
            };
            lint_args.lint.fix = true; // term fix is always lint --fix
            handle_lint(lint_args, ctx)
        }
        Some(TermSubcommand::Show(args)) => handle_show(args, ctx),
        Some(TermSubcommand::Remove(args)) => handle_remove(args, ctx),
        Some(TermSubcommand::Rename(args)) => handle_rename(args, ctx),
        Some(TermSubcommand::Correction(cmd)) => handle_correction(cmd, ctx),
        Some(TermSubcommand::Move(args)) => handle_move(args, ctx),
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

fn update_input<'a>(args: &'a TermUpdateArgs) -> term_svc::TermUpdate<'a> {
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
        add_corrections: &args.add_correction,
        delete_corrections: &args.delete_correction,
        correction_matches: &args.correction_match,
        correction_fixes: &args.correction_fix,
        correction_pinyins: &args.correction_pinyin,
    }
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
    let filter_scope = args.scope.as_deref().unwrap_or("all");

    let (terms, scope_map) = if let Some(project_name) = ctx.project() {
        let project_path = svc_util::resolve_project(root, Some(project_name), ctx.cwd())?;
        let mut merged = if filter_scope == "global" {
            Vec::new()
        } else {
            term_svc::list_terms(&project_path, args.filter.as_deref())?
        };
        let global_terms = if filter_scope == "project" {
            Vec::new()
        } else {
            term_svc::global::list_terms(root, args.filter.as_deref())?
        };
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

    // Apply new filters (AND semantics)
    let filtered_terms: Vec<&crate::model::term::Term> = terms
        .iter()
        .filter(|t| {
            // --tag filter: term must have at least one matching tag
            if !args.tag.is_empty() && !args.tag.iter().any(|tag| t.tags.contains(tag)) {
                return false;
            }
            // --alias filter: term must have at least one matching alias
            if !args.alias.is_empty() && !args.alias.iter().any(|alias| t.aliases.contains(alias)) {
                return false;
            }
            // --has-correction filter: term must have at least one correction
            if args.has_correction && t.corrections.is_empty() {
                return false;
            }
            true
        })
        .collect();

    let opts = ListOpts::from_flags(args.no_headers.no_headers, args.no_trunc.no_trunc)
        .with_repo_root(Some(root.to_path_buf()));

    match ctx.format() {
        Format::Json => {
            let items: Vec<serde_json::Value> = filtered_terms
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
            let mut rows = Vec::with_capacity(filtered_terms.len());
            for t in &filtered_terms {
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
                    term_svc::lint_path_with_global(&pp, root, path, true, true, include_suggested, &args.term)?
                } else {
                    term_svc::lint_terms_with_global(&pp, root, true, true, include_suggested, &args.term)?
                }
            } else if let Some(ref path) = args.path {
                term_svc::global::lint_path(root, path, true, true, include_suggested, &args.term)?
            } else {
                term_svc::global::lint_terms(root, true, true, include_suggested, &args.term)?
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

    // Determine effective target type for output
    let target_type: &str = if args.article.is_some() {
        "article"
    } else if args.path.is_some() {
        "file"
    } else {
        "project"
    };

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
                &args.term,
            )?
        } else {
            term_svc::lint_terms_with_global(
                &project_path,
                root,
                effective_fix,
                effective_dry_run,
                include_suggested,
                &args.term,
            )?
        }
    } else if let Some(ref path) = args.path {
        let resolved = svc_util::path::resolve_lint_path(path, None, cwd, root)?;
        term_svc::global::lint_path(
            root,
            &resolved.to_string_lossy(),
            effective_fix,
            effective_dry_run,
            include_suggested,
            &args.term,
        )?
    } else {
        term_svc::global::lint_terms(root, effective_fix, effective_dry_run, include_suggested, &args.term)?
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
            data.insert("target_type".to_string(), serde_json::Value::String(target_type.to_string()));
            data.insert(
                "term_filter".to_string(),
                serde_json::Value::Array(args.term.iter().map(|n| serde_json::Value::String(n.clone())).collect()),
            );
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
            let output = format_lint_text_with_target(&report, effective_fix, effective_dry_run, Some(target_type));
            if args.term.is_empty() {
                Ok(CommandOutcome::Raw(output, exit_code))
            } else {
                let scoped = format!("scoped to term(s): {}\n{output}", args.term.join(", "));
                Ok(CommandOutcome::Raw(scoped, exit_code))
            }
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
    format_lint_text_with_target(report, fix, dry_run, None)
}

fn format_lint_text_with_target(
    report: &crate::model::term::TermLintReport,
    fix: bool,
    dry_run: bool,
    target_type: Option<&str>,
) -> String {
    let mut lines = Vec::new();

    if let Some(tt) = target_type {
        lines.push(format!("target: {tt}"));
    }

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
    // US1: --misrecognition is unsupported on term update
    if !args.misrecognition.is_empty() {
        return Err(MfError::usage(
            "--misrecognition is not supported on `term update`; use `mf term correction add` to add a correction to an existing term, or `mf term new --misrecognition` when creating a new term",
            Some("use `mf term correction add <TERM> <ORIGINAL> <CORRECT>` to add a correction".to_string()),
        ));
    }
    if args.description.is_some() && args.clear_description {
        return Err(MfError::usage("--description and --clear-description are mutually exclusive", None));
    }
    if args.confidence.is_some() && args.clear_confidence {
        return Err(MfError::usage("--confidence and --clear-confidence are mutually exclusive", None));
    }

    let root = ctx.require_repo_path()?;

    let update = update_input(&args);
    let mut warnings: Vec<String> = Vec::new();

    if args.dry_run.dry_run {
        return handle_update_dry_run(&args, &update, root, ctx, &mut warnings);
    }
    let (term, global_fallback) = if let Some(project_name) = ctx.project() {
        let project_path = svc_util::resolve_project(root, Some(project_name), ctx.cwd())?;
        match term_svc::fix_term(&project_path, &args.term, update) {
            Ok(term) => (term, false),
            Err(MfError::NotFound { .. }) => {
                // Try global fallback; emit the WARN only when that write actually succeeds.
                let term = term_svc::global::fix_term(root, &args.term, update)?;
                emit_scope_fallback_warning(project_name, &args.term, &mut warnings);
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
    if !args.add_correction.is_empty() {
        changes.insert("corrections".to_string(), serde_json::json!({"added": args.add_correction}));
    }
    if !args.delete_correction.is_empty() {
        changes.insert("corrections".to_string(), serde_json::json!({"deleted": args.delete_correction}));
    }
    if !args.correction_match.is_empty() {
        changes.insert("correction_match".to_string(), serde_json::json!(args.correction_match));
    }
    if !args.correction_fix.is_empty() {
        changes.insert("correction_fix".to_string(), serde_json::json!(args.correction_fix));
    }
    if !args.correction_pinyin.is_empty() {
        changes.insert("correction_pinyin".to_string(), serde_json::json!(args.correction_pinyin));
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

// ── Handle: mf term update (dry-run) ─────────────────────────────────────

fn handle_update_dry_run(
    args: &TermUpdateArgs,
    update: &term_svc::TermUpdate<'_>,
    root: &std::path::Path,
    ctx: &CommandCtx,
    warnings: &mut Vec<String>,
) -> Result<CommandOutcome> {
    // Resolve which term would be targeted (project or global)
    let scope = resolve_update_target(root, ctx, &args.term)?;

    // Build planned changes list
    let mut planned = Vec::new();
    if update.definition.is_some() {
        planned.push("definition".to_string());
    }
    if update.description.is_some() || update.clear_description {
        planned.push("description".to_string());
    }
    if update.confidence.is_some() || update.clear_confidence {
        planned.push("confidence".to_string());
    }
    if !update.aliases.is_empty() {
        planned.push(format!("add {} alias(es)", update.aliases.len()));
    }
    if !update.tags.is_empty() {
        planned.push(format!("add {} tag(s)", update.tags.len()));
    }
    if !update.delete_aliases.is_empty() {
        planned.push(format!("remove {} alias(es)", update.delete_aliases.len()));
    }
    if !update.delete_tags.is_empty() {
        planned.push(format!("remove {} tag(s)", update.delete_tags.len()));
    }

    let scope_str = scope.as_str();
    if scope_str == "global" {
        if let Some(pn) = ctx.project() {
            emit_scope_fallback_warning(pn, &args.term, warnings);
        }
    }

    let result = VerbResult {
        verb: Verb::Update,
        kind: "term",
        identity: args.term.clone(),
        old_identity: None,
        path: None,
        dry_run: true,
        details: serde_json::json!({
            "scope": scope_str,
            "changes": planned,
        }),
    };

    match ctx.format() {
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), warnings.clone(), None)),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root)))),
            warnings.clone(),
            None,
        )),
    }
}

/// Resolve which scope an update would target (project or global) without
/// writing anything. The `show_term` lookups also validate that the term
/// exists, propagating a not-found error when it does not.
fn resolve_update_target(root: &std::path::Path, ctx: &CommandCtx, term_name: &str) -> Result<term_svc::WriteScope> {
    use term_svc::WriteScope;

    if let Some(project_name) = ctx.project() {
        let project_path = crate::service::util::resolve_project(root, Some(project_name), ctx.cwd())?;
        match term_svc::show_term(&project_path, term_name) {
            Ok(_) => Ok(WriteScope::Project(project_path)),
            Err(MfError::NotFound { .. }) => {
                term_svc::global::show_term(root, term_name)?;
                Ok(WriteScope::Global(root.to_path_buf()))
            }
            Err(e) => Err(e),
        }
    } else {
        term_svc::global::show_term(root, term_name)?;
        Ok(WriteScope::Global(root.to_path_buf()))
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
                emit_scope_fallback_warning(project_name, &args.term, &mut warnings);
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
                emit_scope_fallback_warning(project_name, &args.old_term, &mut rename_warnings);
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

// ── Correction subcommand dispatch ──────────────────────────────────────────

fn parse_opt_match(raw: Option<&str>) -> Result<Option<MatchKind>> {
    match raw {
        None => Ok(None),
        Some(s) => Ok(Some(parse_match_kind(s)?)),
    }
}

fn parse_opt_fix(raw: Option<&str>) -> Result<Option<FixKind>> {
    match raw {
        None => Ok(None),
        Some(s) => Ok(Some(parse_fix_kind(s)?)),
    }
}

fn parse_opt_boundary(raw: Option<&str>) -> Result<Option<Boundary>> {
    match raw {
        None => Ok(None),
        Some(s) => Ok(Some(parse_boundary(s)?)),
    }
}

fn handle_correction(cmd: TermCorrectionCmd, ctx: &CommandCtx) -> Result<CommandOutcome> {
    use TermCorrectionSubcommand::*;
    match cmd.command {
        Add(args) => handle_correction_add(args, ctx),
        List(args) => handle_correction_list(args, ctx),
        Show(args) => handle_correction_show(args, ctx),
        Update(args) => handle_correction_update(args, ctx),
        Remove(args) => handle_correction_remove(args, ctx),
    }
}

fn correction_scope(root: &std::path::Path, ctx: &CommandCtx) -> Result<(String, std::path::PathBuf)> {
    if let Some(pn) = ctx.project() {
        let pp = crate::service::util::resolve_project(root, Some(pn), ctx.cwd())?;
        Ok(("project".to_string(), pp))
    } else {
        Ok(("global".to_string(), root.to_path_buf()))
    }
}

fn handle_correction_add(args: TermCorrectionAddArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    let (scope, scope_path) = correction_scope(root, ctx)?;
    let warnings: Vec<String> = Vec::new();

    let (corr, created) = if scope == "project" {
        term_svc::correction::add_correction(
            &scope_path,
            &args.term,
            &args.original,
            &args.correct,
            parse_opt_match(args.r#match.as_deref())?,
            parse_opt_fix(args.fix.as_deref())?,
            parse_opt_boundary(args.boundary.as_deref())?,
            args.pinyin.map(|s| if s.is_empty() { None } else { Some(s) }),
        )?
    } else {
        term_svc::correction::add_correction_global(
            &scope_path,
            &args.term,
            &args.original,
            &args.correct,
            parse_opt_match(args.r#match.as_deref())?,
            parse_opt_fix(args.fix.as_deref())?,
            parse_opt_boundary(args.boundary.as_deref())?,
            args.pinyin.map(|s| if s.is_empty() { None } else { Some(s) }),
        )?
    };

    let data = serde_json::json!({
        "term": args.term,
        "scope": scope,
        "created": created,
        "correction": serde_json::to_value(&corr).unwrap_or_default(),
    });

    match ctx.format() {
        Format::Json => Ok(CommandOutcome::Success(data, warnings, None)),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(if created {
                format!("added correction \"{}\" → \"{}\" to term \"{}\"", corr.original, corr.correct, args.term)
            } else {
                format!(
                    "correction \"{}\" → \"{}\" already exists on term \"{}\", skipped",
                    corr.original, corr.correct, args.term
                )
            }),
            warnings,
            None,
        )),
    }
}

fn handle_correction_list(args: TermCorrectionListArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    let (scope, scope_path) = correction_scope(root, ctx)?;

    let corrections = if scope == "project" {
        term_svc::correction::list_corrections(&scope_path, &args.term)?
    } else {
        term_svc::correction::list_corrections_global(&scope_path, &args.term)?
    };

    match ctx.format() {
        Format::Json => {
            let items: Vec<serde_json::Value> =
                corrections.iter().map(|c| serde_json::to_value(c).unwrap_or_default()).collect();
            Ok(CommandOutcome::Success(
                serde_json::json!({"term": args.term, "scope": scope, "corrections": items}),
                Vec::new(),
                None,
            ))
        }
        Format::Text => {
            if corrections.is_empty() {
                return Ok(CommandOutcome::Raw(format!("no corrections for term \"{}\"", args.term), None));
            }
            let mut lines = vec![format!("corrections for term \"{}\":", args.term)];
            for c in &corrections {
                lines.push(format!(
                    "  \"{}\" → \"{}\" [match={}, fix={}]",
                    c.original,
                    c.correct,
                    match_to_str(&c.r#match),
                    fix_to_str(&c.fix)
                ));
            }
            Ok(CommandOutcome::Raw(lines.join("\n"), None))
        }
    }
}

fn handle_correction_show(args: TermCorrectionShowArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    let (scope, scope_path) = correction_scope(root, ctx)?;

    let corr = if scope == "project" {
        term_svc::correction::show_correction(&scope_path, &args.term, &args.original)?
    } else {
        term_svc::correction::show_correction_global(&scope_path, &args.term, &args.original)?
    };

    match ctx.format() {
        Format::Json => Ok(CommandOutcome::Success(
            serde_json::json!({"term": args.term, "scope": scope, "correction": serde_json::to_value(&corr).unwrap_or_default()}),
            Vec::new(),
            None,
        )),
        Format::Text => Ok(CommandOutcome::Raw(
            format!(
                "correction \"{}\" → \"{}\"\n  match: {}\n  fix: {}\n  boundary: {}{}",
                corr.original,
                corr.correct,
                match_to_str(&corr.r#match),
                fix_to_str(&corr.fix),
                boundary_to_str(&corr.boundary),
                corr.pinyin.map_or(String::new(), |p| format!("\n  pinyin: {p}")),
            ),
            None,
        )),
    }
}

fn handle_correction_update(args: TermCorrectionUpdateArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    let (scope, scope_path) = correction_scope(root, ctx)?;

    let pinyin_val = args.pinyin.map(|s| if s.is_empty() { None } else { Some(s) });

    let corr = if scope == "project" {
        term_svc::correction::update_correction(
            &scope_path,
            &args.term,
            &args.original,
            args.correct,
            parse_opt_match(args.r#match.as_deref())?,
            parse_opt_fix(args.fix.as_deref())?,
            parse_opt_boundary(args.boundary.as_deref())?,
            pinyin_val,
        )?
    } else {
        term_svc::correction::update_correction_global(
            &scope_path,
            &args.term,
            &args.original,
            args.correct,
            parse_opt_match(args.r#match.as_deref())?,
            parse_opt_fix(args.fix.as_deref())?,
            parse_opt_boundary(args.boundary.as_deref())?,
            pinyin_val,
        )?
    };

    match ctx.format() {
        Format::Json => Ok(CommandOutcome::Success(
            serde_json::json!({"term": args.term, "scope": scope, "correction": serde_json::to_value(&corr).unwrap_or_default()}),
            Vec::new(),
            None,
        )),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(format!("updated correction \"{}\" on term \"{}\"", args.original, args.term)),
            Vec::new(),
            None,
        )),
    }
}

fn handle_correction_remove(args: TermCorrectionRemoveArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    let (scope, scope_path) = correction_scope(root, ctx)?;
    let warnings: Vec<String> = Vec::new();

    if args.dry_run.dry_run {
        let result = VerbResult {
            verb: Verb::Remove,
            kind: "correction",
            identity: format!("{}/{}", args.term, args.original),
            old_identity: None,
            path: None,
            dry_run: true,
            details: serde_json::json!({"scope": scope}),
        };
        return match ctx.format() {
            Format::Json => Ok(CommandOutcome::Success(verb_json(&result), warnings, None)),
            Format::Text => Ok(CommandOutcome::Success(
                serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root)))),
                warnings,
                None,
            )),
        };
    }

    let corr = if scope == "project" {
        term_svc::correction::remove_correction(&scope_path, &args.term, &args.original)?
    } else {
        term_svc::correction::remove_correction_global(&scope_path, &args.term, &args.original)?
    };

    match ctx.format() {
        Format::Json => Ok(CommandOutcome::Success(
            serde_json::json!({"term": args.term, "scope": scope, "removed": serde_json::to_value(&corr).unwrap_or_default()}),
            warnings,
            None,
        )),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(format!(
                "removed correction \"{}\" → \"{}\" from term \"{}\"",
                corr.original, corr.correct, args.term
            )),
            warnings,
            None,
        )),
    }
}

fn match_to_str(m: &MatchKind) -> &str {
    match m {
        MatchKind::Word => "word",
        MatchKind::Substring => "substring",
        MatchKind::Pinyin => "pinyin",
    }
}

fn fix_to_str(f: &FixKind) -> &str {
    match f {
        FixKind::Required => "required",
        FixKind::Suggested => "suggested",
    }
}

fn boundary_to_str(b: &Boundary) -> &str {
    match b {
        Boundary::Loose => "loose",
        Boundary::Standalone => "standalone",
    }
}

// ── Handle: mf term move / mv ──────────────────────────────────────────────

fn handle_move(args: TermMoveArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    let warnings: Vec<String> = Vec::new();

    let dst_project = args.to_project.as_deref();

    if !args.to_global && dst_project.is_none() {
        return Err(MfError::usage("must specify --to-global or --to-project <PROJECT> for the destination", None));
    }
    // Reject early so dry-run and real runs reject identical inputs.
    if args.from_global && args.to_global {
        return Err(MfError::usage("source and destination are both global; nothing to do", None));
    }

    // Resolve source path
    let src_path = if args.from_global {
        root.to_path_buf()
    } else if let Some(pn) = ctx.project() {
        crate::service::util::resolve_project(root, Some(pn), ctx.cwd())?
    } else {
        root.to_path_buf()
    };

    // Resolve destination path
    let dst_path = if args.to_global {
        root.to_path_buf()
    } else if let Some(pn) = dst_project {
        crate::service::util::resolve_project(root, Some(pn), ctx.cwd())?
    } else {
        root.to_path_buf()
    };

    if args.dry_run.dry_run {
        let result = VerbResult {
            verb: Verb::Move,
            kind: "term",
            identity: args.term.clone(),
            old_identity: None,
            path: None,
            dry_run: true,
            details: serde_json::json!({
                "from_scope": if args.from_global { "global" } else { "project" },
                "to_scope": if args.to_global { "global" } else { "project" },
                "force": args.force.force,
            }),
        };
        return match ctx.format() {
            Format::Json => Ok(CommandOutcome::Success(verb_json(&result), warnings, None)),
            Format::Text => Ok(CommandOutcome::Success(
                serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root)))),
                warnings,
                None,
            )),
        };
    }

    let outcome = if args.to_global {
        term_svc::move_::move_project_to_global(&src_path, &dst_path, &args.term, args.force.force)?
    } else if args.from_global {
        term_svc::move_::move_global_to_project(&src_path, &dst_path, &args.term, args.force.force)?
    } else {
        term_svc::move_::move_project_to_project(&src_path, &dst_path, &args.term, args.force.force)?
    };

    let result = VerbResult {
        verb: Verb::Move,
        kind: "term",
        identity: outcome.term.term.clone(),
        old_identity: None,
        path: None,
        dry_run: false,
        details: serde_json::json!({
            "from_scope": outcome.from_scope,
            "to_scope": outcome.to_scope,
            "side_effects": outcome.side_effects,
        }),
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
