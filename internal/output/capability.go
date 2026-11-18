package output

import (
	"context"
	"os"
	"os/exec"
	"strings"
	"time"

	"golang.org/x/term"
)

type TerminalProfile struct {
	StdoutIsTTY     bool    `json:"stdout_is_tty"`
	TerminalWidth   int     `json:"terminal_width"`
	Term            *string `json:"term,omitempty"`
	TermProgram     *string `json:"term_program,omitempty"`
	ColorMode       string  `json:"color_mode"`
	Truecolor       bool    `json:"truecolor"`
	Hyperlinks      bool    `json:"hyperlinks"`
	Styling         bool    `json:"styling"`
	DetectionSource string  `json:"detection_source"`
	FallbackReason  *string `json:"fallback_reason,omitempty"`
	OverrideSource  *string `json:"override_source,omitempty"`
}

type OutputPolicy struct {
	Format                 string `json:"format"`
	PlainOutput            bool   `json:"plain_output"`
	EmitANSIColor          bool   `json:"emit_ansi_color"`
	EmitTruecolor          bool   `json:"emit_truecolor"`
	EmitHyperlinks         bool   `json:"emit_hyperlinks"`
	PreserveVisibleTargets bool   `json:"preserve_visible_targets"`
	DiagnosticsToStderr    bool   `json:"diagnostics_to_stderr"`
}

type CapabilityInput struct {
	StdoutIsTTY bool
	Width       int
	Env         map[string]string
	Terminfo    string
}

func BuildTerminalProfile(input CapabilityInput) TerminalProfile {
	env := input.Env
	present := func(name string) *string {
		if value, ok := env[name]; ok {
			return &value
		}
		return nil
	}
	profile := TerminalProfile{StdoutIsTTY: input.StdoutIsTTY || env["MF_FORCE_TTY"] != "", TerminalWidth: input.Width, Term: present("TERM"), TermProgram: present("TERM_PROGRAM"), ColorMode: "none", DetectionSource: "disabled"}
	if profile.TerminalWidth == 0 {
		profile.TerminalWidth = 200
	}
	if env["NO_COLOR"] != "" || !profile.StdoutIsTTY {
		reason := "stdout is not a terminal"
		if env["NO_COLOR"] != "" {
			reason = "NO_COLOR active"
		}
		profile.FallbackReason = &reason
		return profile
	}
	if env["MF_FORCE_TTY"] != "" {
		source := "MF_FORCE_TTY"
		profile.OverrideSource = &source
	}
	terminal, program, colorterm := env["TERM"], env["TERM_PROGRAM"], env["COLORTERM"]
	source := ""
	switch {
	case strings.EqualFold(colorterm, "truecolor") || strings.EqualFold(colorterm, "24bit"):
		profile.Truecolor = true
		source = "terminal_identity"
	case strings.Contains(terminal, "ghostty") || strings.Contains(terminal, "kitty"):
		profile.Truecolor = true
		source = "terminal_identity"
	case strings.Contains(terminal, "-direct") || strings.Contains(terminal, "truecolor"):
		profile.Truecolor = true
		source = "terminfo"
	case strings.HasPrefix(terminal, "tmux") || strings.HasPrefix(terminal, "screen") && env["TMUX"] != "":
		profile.Truecolor = true
		source = "terminal_identity"
	case strings.Contains(input.Terminfo, "RGB") || strings.Contains(input.Terminfo, "Tc"):
		profile.Truecolor = true
		source = "terminfo"
	}
	if env["MF_NO_HYPERLINKS"] == "" {
		profile.Hyperlinks = env["MF_FORCE_HYPERLINKS"] != ""
		for _, line := range strings.Split(input.Terminfo, "\n") {
			if strings.TrimSpace(line) == "Ms" {
				profile.Hyperlinks = true
			}
		}
		known := []string{"ghostty", "kitty", "wezterm", "alacritty", "iterm.app", "iterm2", "vscode", "warpterminal", "warp", "apple_terminal", "hyper", "rio", "tabby", "windsurf", "cursor", "contour", "foot"}
		for _, identity := range known {
			if strings.Contains(strings.ToLower(terminal), identity) || strings.Contains(strings.ToLower(program), identity) {
				profile.Hyperlinks = true
			}
		}
		if strings.HasPrefix(terminal, "tmux") || strings.HasPrefix(terminal, "screen") {
			profile.Hyperlinks = true
		}
	}
	isDumb := profile.Term != nil && (terminal == "dumb" || terminal == "")
	if source == "" {
		source = "terminal_identity"
		if isDumb {
			source = "fallback"
		}
	}
	profile.DetectionSource = source
	_, hasColorterm := env["COLORTERM"]
	switch {
	case profile.Truecolor:
		profile.ColorMode = "truecolor"
	case strings.Contains(terminal, "256color"):
		profile.ColorMode = "ansi256"
	case isDumb:
		reason := "TERM=" + terminal + " does not support color"
		profile.FallbackReason = &reason
		profile.DetectionSource = "fallback"
	case hasColorterm || strings.Contains(terminal, "color") || strings.HasPrefix(terminal, "xterm") || strings.HasPrefix(terminal, "screen"):
		profile.ColorMode = "ansi16"
		if profile.Hyperlinks {
			profile.ColorMode = "ansi256"
		}
	default:
		profile.ColorMode = "ansi16"
		profile.DetectionSource = "fallback"
	}
	profile.Styling = profile.ColorMode != "none"
	return profile
}

func RenderingPolicy(profile TerminalProfile, format string) OutputPolicy {
	policy := OutputPolicy{Format: format, PlainOutput: true, PreserveVisibleTargets: true, DiagnosticsToStderr: true}
	if format == "json" || !profile.StdoutIsTTY || !profile.Styling {
		return policy
	}
	policy.PlainOutput = false
	policy.EmitANSIColor = profile.ColorMode != "none"
	policy.EmitTruecolor = profile.Truecolor
	policy.EmitHyperlinks = profile.Hyperlinks
	return policy
}

// ProbeTerminal is called once by the command boundary; renderers receive the
// resulting policy rather than probing terminal state for each row or field.
func ProbeTerminal(stdout *os.File) TerminalProfile {
	env := map[string]string{}
	for _, name := range []string{"TERM", "TERM_PROGRAM", "COLORTERM", "TMUX", "NO_COLOR", "MF_FORCE_TTY", "MF_NO_HYPERLINKS", "MF_FORCE_HYPERLINKS"} {
		if value, ok := os.LookupEnv(name); ok {
			env[name] = value
		}
	}
	input := CapabilityInput{StdoutIsTTY: term.IsTerminal(int(stdout.Fd())), Env: env, Width: 200}
	if width, _, err := term.GetSize(int(stdout.Fd())); err == nil {
		input.Width = width
	}
	if (input.StdoutIsTTY || env["MF_FORCE_TTY"] != "") && env["NO_COLOR"] == "" {
		ctx, cancel := context.WithTimeout(context.Background(), 100*time.Millisecond)
		defer cancel()
		if raw, err := exec.CommandContext(ctx, "infocmp", "-1").Output(); err == nil {
			input.Terminfo = string(raw)
		}
	}
	return BuildTerminalProfile(input)
}
