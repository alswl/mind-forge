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

use self::correction::handle_correction;
use self::lint::handle_lint;
use self::list::handle_list;
use self::move_::handle_move;
use self::new::handle_new;
use self::remove::handle_remove;
use self::rename::handle_rename;
use self::show::handle_show;
use self::update::handle_update;

mod correction;
mod lint;
mod list;
mod move_;
mod new;
mod remove;
mod rename;
mod show;
mod update;

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
