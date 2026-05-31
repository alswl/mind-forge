use serde::Serialize;

/// Progressive terminal color support levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorMode {
    None,
    Ansi16,
    Ansi256,
    Truecolor,
}

/// How the current capability profile was determined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum DetectionSource {
    Disabled,
    NonTty,
    EnvOverride,
    Terminfo,
    TerminalIdentity,
    Fallback,
}

/// Output format used for serialization in the diagnostic report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Text,
    Json,
}

/// The effective terminal capability set for a single `mf` process invocation.
#[derive(Debug, Clone, Serialize)]
pub struct TerminalCapabilityProfile {
    pub stdout_is_tty: bool,
    pub terminal_width: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub term: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub term_program: Option<String>,
    pub color_mode: ColorMode,
    pub truecolor: bool,
    pub hyperlinks: bool,
    pub styling: bool,
    pub detection_source: DetectionSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub override_source: Option<String>,
}

impl TerminalCapabilityProfile {
    #[allow(dead_code)]
    pub fn disabled(width: usize) -> Self {
        Self {
            stdout_is_tty: false,
            terminal_width: width,
            term: None,
            term_program: None,
            color_mode: ColorMode::None,
            truecolor: false,
            hyperlinks: false,
            styling: false,
            detection_source: DetectionSource::Disabled,
            fallback_reason: None,
            override_source: None,
        }
    }
}

/// Maps a terminal profile and command output mode to renderer behavior.
#[derive(Debug, Clone, Serialize)]
pub struct OutputRenderingPolicy {
    pub format: OutputFormat,
    pub plain_output: bool,
    pub emit_ansi_color: bool,
    pub emit_truecolor: bool,
    pub emit_hyperlinks: bool,
    pub preserve_visible_targets: bool,
    pub diagnostics_to_stderr: bool,
}

impl OutputRenderingPolicy {
    pub fn plain(format: OutputFormat) -> Self {
        Self {
            format,
            plain_output: true,
            emit_ansi_color: false,
            emit_truecolor: false,
            emit_hyperlinks: false,
            preserve_visible_targets: true,
            diagnostics_to_stderr: true,
        }
    }

    pub fn from_profile(profile: &TerminalCapabilityProfile, format: OutputFormat) -> Self {
        if format == OutputFormat::Json || !profile.stdout_is_tty || !profile.styling {
            return Self::plain(format);
        }
        Self {
            format,
            plain_output: false,
            emit_ansi_color: profile.color_mode != ColorMode::None,
            emit_truecolor: profile.truecolor,
            emit_hyperlinks: profile.hyperlinks,
            preserve_visible_targets: true,
            diagnostics_to_stderr: true,
        }
    }
}

/// A single diagnostic check result.
#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticCheck {
    pub name: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Selected non-secret terminal environment variables.
#[derive(Debug, Clone, Serialize)]
pub struct EnvironmentSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub term: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colorterm: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub term_program: Option<String>,
    pub tmux: bool,
    pub no_color: bool,
}

/// Serializable result returned by `mf config terminal`.
#[derive(Debug, Clone, Serialize)]
pub struct CapabilityDiagnosticReport {
    pub profile: TerminalCapabilityProfile,
    pub policy: OutputRenderingPolicy,
    pub environment: EnvironmentSummary,
    pub checks: Vec<DiagnosticCheck>,
    pub recommendations: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_mode_serialize_snake_case() {
        assert_eq!(serde_json::to_value(ColorMode::None).unwrap(), serde_json::json!("none"));
        assert_eq!(serde_json::to_value(ColorMode::Ansi16).unwrap(), serde_json::json!("ansi16"));
        assert_eq!(serde_json::to_value(ColorMode::Ansi256).unwrap(), serde_json::json!("ansi256"));
        assert_eq!(serde_json::to_value(ColorMode::Truecolor).unwrap(), serde_json::json!("truecolor"));
    }

    #[test]
    fn detection_source_serialize_snake_case() {
        assert_eq!(serde_json::to_value(DetectionSource::Disabled).unwrap(), serde_json::json!("disabled"));
        assert_eq!(serde_json::to_value(DetectionSource::NonTty).unwrap(), serde_json::json!("non_tty"));
        assert_eq!(serde_json::to_value(DetectionSource::EnvOverride).unwrap(), serde_json::json!("env_override"));
        assert_eq!(serde_json::to_value(DetectionSource::Terminfo).unwrap(), serde_json::json!("terminfo"));
        assert_eq!(
            serde_json::to_value(DetectionSource::TerminalIdentity).unwrap(),
            serde_json::json!("terminal_identity")
        );
        assert_eq!(serde_json::to_value(DetectionSource::Fallback).unwrap(), serde_json::json!("fallback"));
    }

    #[test]
    fn disabled_profile_has_no_capabilities() {
        let p = TerminalCapabilityProfile::disabled(80);
        assert!(!p.stdout_is_tty);
        assert_eq!(p.color_mode, ColorMode::None);
        assert!(!p.truecolor);
        assert!(!p.hyperlinks);
        assert!(!p.styling);
        assert_eq!(p.detection_source, DetectionSource::Disabled);
    }

    #[test]
    fn disabled_profile_serialization() {
        let p = TerminalCapabilityProfile::disabled(80);
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["stdout_is_tty"], false);
        assert_eq!(v["color_mode"], "none");
        assert_eq!(v["truecolor"], false);
        assert_eq!(v["hyperlinks"], false);
        assert_eq!(v["styling"], false);
        assert_eq!(v["detection_source"], "disabled");
    }

    #[test]
    fn profile_skips_none_optionals_in_json() {
        let p = TerminalCapabilityProfile::disabled(80);
        let json = serde_json::to_string(&p).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(v.get("term").is_none());
        assert!(v.get("term_program").is_none());
        assert!(v.get("fallback_reason").is_none());
        assert!(v.get("override_source").is_none());
    }

    #[test]
    fn policy_json_forces_plain_output() {
        let p = OutputRenderingPolicy::plain(OutputFormat::Json);
        assert!(p.plain_output);
        assert!(!p.emit_ansi_color);
        assert!(!p.emit_truecolor);
        assert!(!p.emit_hyperlinks);
    }

    #[test]
    fn policy_text_with_rich_profile_enables_features() {
        let profile = TerminalCapabilityProfile {
            stdout_is_tty: true,
            terminal_width: 120,
            term: Some("xterm-ghostty".into()),
            term_program: Some("Ghostty".into()),
            color_mode: ColorMode::Truecolor,
            truecolor: true,
            hyperlinks: true,
            styling: true,
            detection_source: DetectionSource::Terminfo,
            fallback_reason: None,
            override_source: None,
        };
        let policy = OutputRenderingPolicy::from_profile(&profile, OutputFormat::Text);
        assert!(!policy.plain_output);
        assert!(policy.emit_ansi_color);
        assert!(policy.emit_truecolor);
        assert!(policy.emit_hyperlinks);
        assert!(policy.preserve_visible_targets);
    }

    #[test]
    fn policy_json_format_disables_rich_output() {
        let profile = TerminalCapabilityProfile {
            stdout_is_tty: true,
            terminal_width: 120,
            term: Some("xterm-ghostty".into()),
            term_program: Some("Ghostty".into()),
            color_mode: ColorMode::Truecolor,
            truecolor: true,
            hyperlinks: true,
            styling: true,
            detection_source: DetectionSource::Terminfo,
            fallback_reason: None,
            override_source: None,
        };
        let policy = OutputRenderingPolicy::from_profile(&profile, OutputFormat::Json);
        assert!(policy.plain_output);
        assert!(!policy.emit_ansi_color);
        assert!(!policy.emit_truecolor);
        assert!(!policy.emit_hyperlinks);
    }

    #[test]
    fn environment_summary_serialization() {
        let env = EnvironmentSummary {
            term: Some("xterm-ghostty".into()),
            colorterm: Some("truecolor".into()),
            term_program: Some("Ghostty".into()),
            tmux: false,
            no_color: false,
        };
        let v = serde_json::to_value(&env).unwrap();
        assert_eq!(v["term"], "xterm-ghostty");
        assert_eq!(v["colorterm"], "truecolor");
        assert_eq!(v["term_program"], "Ghostty");
        assert_eq!(v["tmux"], false);
        assert_eq!(v["no_color"], false);
    }

    #[test]
    fn diagnostic_report_serialization() {
        let report = CapabilityDiagnosticReport {
            profile: TerminalCapabilityProfile::disabled(80),
            policy: OutputRenderingPolicy::plain(OutputFormat::Text),
            environment: EnvironmentSummary {
                term: None,
                colorterm: None,
                term_program: None,
                tmux: false,
                no_color: true,
            },
            checks: vec![DiagnosticCheck {
                name: "stdout_tty".into(),
                status: "fail".into(),
                detail: Some("stdout is not a terminal".into()),
            }],
            recommendations: vec![],
        };
        let v = serde_json::to_value(&report).unwrap();
        assert_eq!(v["profile"]["color_mode"], "none");
        assert_eq!(v["policy"]["plain_output"], true);
        assert_eq!(v["environment"]["no_color"], true);
        assert_eq!(v["checks"][0]["name"], "stdout_tty");
        assert_eq!(v["checks"][0]["status"], "fail");
    }
}
