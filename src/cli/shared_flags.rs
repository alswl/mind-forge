use clap::Args;
use serde::Serialize;

#[derive(Debug, Clone, Args, Serialize)]
pub struct DryRunFlag {
    #[arg(long = "dry-run", help = "Preview changes without writing")]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ForceFlag {
    #[arg(
        short = 'f',
        long = "force",
        help = "Proceed despite safety checks: overwrite an existing target, or remove an entity referenced by others"
    )]
    pub force: bool,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct YesFlag {
    #[arg(short = 'y', long = "yes", help = "Skip interactive confirmation prompt")]
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
    #[arg(long = "include-suggested", help = "Apply all corrections including suggested")]
    pub include_suggested: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    /// Minimal wrapper so we can test each flag struct in isolation.
    #[derive(Debug, Parser)]
    struct DryRunTest {
        #[command(flatten)]
        pub flag: DryRunFlag,
    }

    #[derive(Debug, Parser)]
    struct ForceTest {
        #[command(flatten)]
        pub flag: ForceFlag,
    }

    #[derive(Debug, Parser)]
    struct YesTest {
        #[command(flatten)]
        pub flag: YesFlag,
    }

    #[test]
    fn dry_run_flag_long_name() {
        let args = DryRunTest::try_parse_from(["test", "--dry-run"]).unwrap();
        assert!(args.flag.dry_run);
    }

    #[test]
    fn dry_run_flag_no_short() {
        // `-d` should be unknown for DryRunFlag (no short defined)
        let err = DryRunTest::try_parse_from(["test", "-d"]).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("'-d'") || msg.contains("-d"), "unexpected error: {msg}");
    }

    #[test]
    fn dry_run_flag_serde_key() {
        let flag = DryRunFlag { dry_run: true };
        let json = serde_json::to_value(&flag).unwrap();
        assert_eq!(json["dry_run"], serde_json::Value::Bool(true));
    }

    #[test]
    fn force_flag_long_name() {
        let args = ForceTest::try_parse_from(["test", "--force"]).unwrap();
        assert!(args.flag.force);
    }

    #[test]
    fn force_flag_short_f() {
        let args = ForceTest::try_parse_from(["test", "-f"]).unwrap();
        assert!(args.flag.force);
    }

    #[test]
    fn force_flag_serde_key() {
        let flag = ForceFlag { force: true };
        let json = serde_json::to_value(&flag).unwrap();
        assert_eq!(json["force"], serde_json::Value::Bool(true));
    }

    #[test]
    fn yes_flag_long_name() {
        let args = YesTest::try_parse_from(["test", "--yes"]).unwrap();
        assert!(args.flag.yes);
    }

    #[test]
    fn yes_flag_short_y() {
        let args = YesTest::try_parse_from(["test", "-y"]).unwrap();
        assert!(args.flag.yes);
    }

    #[test]
    fn yes_flag_serde_key() {
        let flag = YesFlag { yes: true };
        let json = serde_json::to_value(&flag).unwrap();
        assert_eq!(json["yes"], serde_json::Value::Bool(true));
    }
}
