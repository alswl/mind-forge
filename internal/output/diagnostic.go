package output

import "fmt"

type DiagnosticCheck struct {
	Name   string `json:"name"`
	Status string `json:"status"`
	Detail string `json:"detail,omitempty"`
}

type EnvironmentSummary struct {
	Term        *string `json:"term,omitempty"`
	Colorterm   *string `json:"colorterm,omitempty"`
	TermProgram *string `json:"term_program,omitempty"`
	Tmux        bool    `json:"tmux"`
	NoColor     bool    `json:"no_color"`
}

type CapabilityDiagnosticReport struct {
	Profile         TerminalProfile    `json:"profile"`
	Policy          OutputPolicy       `json:"policy"`
	Environment     EnvironmentSummary `json:"environment"`
	Checks          []DiagnosticCheck  `json:"checks"`
	Recommendations []string           `json:"recommendations"`
}

func TerminalDiagnostics(profile TerminalProfile, format string, env map[string]string) CapabilityDiagnosticReport {
	optional := func(name string) *string {
		value, ok := env[name]
		if !ok {
			return nil
		}
		return &value
	}
	report := CapabilityDiagnosticReport{
		Profile: profile, Policy: RenderingPolicy(profile, format),
		Environment: EnvironmentSummary{Term: optional("TERM"), Colorterm: optional("COLORTERM"), TermProgram: optional("TERM_PROGRAM"), Tmux: env["TMUX"] != "", NoColor: env["NO_COLOR"] != ""},
		Checks:      []DiagnosticCheck{}, Recommendations: []string{},
	}
	for _, check := range []struct {
		name            string
		on              bool
		status, yes, no string
	}{
		{"stdout_tty", profile.StdoutIsTTY, "fail", "stdout is interactive", "stdout is not a terminal"},
		{"truecolor", profile.Truecolor, "info", "truecolor support detected", "no truecolor evidence found"},
		{"hyperlinks", profile.Hyperlinks, "info", "OSC 8 hyperlinks supported", "OSC 8 hyperlinks not detected"},
	} {
		status, detail := check.status, check.no
		if check.on {
			status, detail = "pass", check.yes
		}
		report.Checks = append(report.Checks, DiagnosticCheck{Name: check.name, Status: status, Detail: detail})
	}
	if profile.FallbackReason != nil {
		report.Recommendations = append(report.Recommendations, "Fallback active: "+*profile.FallbackReason)
	}
	if !profile.StdoutIsTTY {
		report.Recommendations = append(report.Recommendations, "Run with MF_FORCE_TTY=1 to test terminal behavior in pipes")
	} else if !profile.Truecolor {
		report.Recommendations = append(report.Recommendations, "Set COLORTERM=truecolor for richer color output")
	}
	return report
}

func (report CapabilityDiagnosticReport) Text() string {
	profile := report.Profile
	term, fallback := "unknown", "-"
	if profile.Term != nil {
		term = *profile.Term
	}
	if profile.FallbackReason != nil {
		fallback = *profile.FallbackReason
	}
	yesNo := func(value bool) string {
		if value {
			return "yes"
		}
		return "no"
	}
	color := map[string]string{"none": "none", "ansi16": "16-color", "ansi256": "256-color", "truecolor": "truecolor"}[profile.ColorMode]
	return fmt.Sprintf("Terminal: %s\nTTY: %s\nColor: %s\nHyperlinks: %s\nFallback: %s\n", term, yesNo(profile.StdoutIsTTY), color, yesNo(profile.Hyperlinks), fallback)
}
