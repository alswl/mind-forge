use clap::Args;
use serde::Serialize;

#[derive(Debug, Clone, Args, Serialize)]
pub struct DryRunFlag {
    #[arg(long = "dry-run", help = "Preview the action without modifying the filesystem")]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ForceFlag {
    #[arg(short = 'f', long = "force", help = "Skip safety checks; not-found becomes a no-op")]
    pub force: bool,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct YesFlag {
    #[arg(short = 'y', long = "yes", help = "Skip the interactive confirmation prompt")]
    pub yes: bool,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct NoHeadersFlag {
    #[arg(long = "no-headers", help = "Suppress the table header row")]
    pub no_headers: bool,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct NoTruncFlag {
    #[arg(long = "no-trunc", help = "Disable column truncation")]
    pub no_trunc: bool,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct LintFlags {
    #[arg(long = "fix", help = "Auto-fix issues that have a fixer")]
    pub fix: bool,
    #[arg(long = "rule", value_name = "RULE", help = "Restrict to a specific lint rule kind")]
    pub rule: Vec<String>,
    #[arg(
        long = "severity",
        value_name = "LEVEL",
        help = "Only emit issues at or above this severity (error|warning|info)"
    )]
    pub severity: Option<String>,
    #[arg(long = "max-warnings", value_name = "N", help = "Exit 1 when warnings exceed this count")]
    pub max_warnings: Option<i32>,
    #[arg(long = "dry-run", help = "Preview fixes without writing (only with --fix)")]
    pub dry_run: bool,
    #[arg(long = "all", help = "Apply all corrections including suggested")]
    pub all: bool,
}
