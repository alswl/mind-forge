package output

import (
	"encoding/json"
	"io"
	"testing"
)

func legacyTerminalJSON(t *testing.T, value any) map[string]any {
	t.Helper()
	raw, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	var got map[string]any
	if err := json.Unmarshal(raw, &got); err != nil {
		t.Fatal(err)
	}
	return got
}

func TestLegacyTerminalModels(t *testing.T) {
	disabled := BuildTerminalProfile(CapabilityInput{Width: 80})
	rich := TerminalProfile{StdoutIsTTY: true, TerminalWidth: 120, ColorMode: "truecolor", Truecolor: true, Hyperlinks: true, Styling: true, DetectionSource: "terminfo"}
	t.Run("color_mode_serialize_snake_case", func(t *testing.T) {
		for _, test := range []struct {
			input CapabilityInput
			want  string
		}{
			{CapabilityInput{Width: 80}, "none"},
			{CapabilityInput{StdoutIsTTY: true, Env: map[string]string{"TERM": "xterm"}}, "ansi16"},
			{CapabilityInput{StdoutIsTTY: true, Env: map[string]string{"TERM": "xterm-256color"}}, "ansi256"},
			{CapabilityInput{StdoutIsTTY: true, Env: map[string]string{"COLORTERM": "truecolor"}}, "truecolor"},
		} {
			if got := legacyTerminalJSON(t, BuildTerminalProfile(test.input))["color_mode"]; got != test.want {
				t.Fatalf("color wire = %v, want %s", got, test.want)
			}
		}
	})
	t.Run("detection_source_serialize_snake_case", func(t *testing.T) {
		for _, source := range []string{"disabled", "non_tty", "env_override", "terminfo", "terminal_identity", "fallback"} {
			if got := legacyTerminalJSON(t, TerminalProfile{DetectionSource: source})["detection_source"]; got != source {
				t.Fatalf("detection source wire = %v, want %s", got, source)
			}
		}
	})
	t.Run("disabled_profile_has_no_capabilities", func(t *testing.T) {
		if disabled.StdoutIsTTY || disabled.ColorMode != "none" || disabled.Truecolor || disabled.Hyperlinks || disabled.Styling || disabled.DetectionSource != "disabled" {
			t.Fatalf("disabled capability profile = %+v", disabled)
		}
	})
	t.Run("disabled_profile_serialization", func(t *testing.T) {
		got := legacyTerminalJSON(t, disabled)
		for key, want := range map[string]any{"stdout_is_tty": false, "color_mode": "none", "truecolor": false, "hyperlinks": false, "styling": false, "detection_source": "disabled"} {
			if got[key] != want {
				t.Fatalf("disabled field %s = %v, want %v", key, got[key], want)
			}
		}
	})
	t.Run("profile_skips_none_optionals_in_json", func(t *testing.T) {
		got := legacyTerminalJSON(t, TerminalProfile{})
		for _, key := range []string{"term", "term_program", "fallback_reason", "override_source"} {
			if _, ok := got[key]; ok {
				t.Fatalf("absent optional %s serialized", key)
			}
		}
	})
	for _, test := range []struct {
		name, format string
		profile      TerminalProfile
		plain        bool
	}{
		{"policy_json_forces_plain_output", "json", TerminalProfile{}, true},
		{"policy_text_with_rich_profile_enables_features", "text", rich, false},
		{"policy_json_format_disables_rich_output", "json", rich, true},
	} {
		t.Run(test.name, func(t *testing.T) {
			got := RenderingPolicy(test.profile, test.format)
			if got.PlainOutput != test.plain || got.EmitANSIColor == test.plain || got.EmitTruecolor == test.plain || got.EmitHyperlinks == test.plain {
				t.Fatalf("rendering policy = %+v", got)
			}
			if !test.plain && !got.PreserveVisibleTargets {
				t.Fatal("visible targets not preserved")
			}
		})
	}
	t.Run("environment_summary_serialization", func(t *testing.T) {
		report := TerminalDiagnostics(disabled, "text", map[string]string{"TERM": "xterm-ghostty", "COLORTERM": "truecolor", "TERM_PROGRAM": "Ghostty"})
		got := legacyTerminalJSON(t, report.Environment)
		for key, want := range map[string]any{"term": "xterm-ghostty", "colorterm": "truecolor", "term_program": "Ghostty", "tmux": false, "no_color": false} {
			if got[key] != want {
				t.Fatalf("environment[%s] = %v, want %v", key, got[key], want)
			}
		}
	})
	t.Run("diagnostic_report_serialization", func(t *testing.T) {
		got := legacyTerminalJSON(t, TerminalDiagnostics(disabled, "text", map[string]string{"NO_COLOR": "1"}))
		if got["profile"].(map[string]any)["color_mode"] != "none" || got["policy"].(map[string]any)["plain_output"] != true || got["environment"].(map[string]any)["no_color"] != true {
			t.Fatalf("diagnostic report fields changed: %#v", got)
		}
		check := got["checks"].([]any)[0].(map[string]any)
		if check["name"] != "stdout_tty" || check["status"] != "fail" {
			t.Fatalf("stdout diagnostic = %#v", check)
		}
	})
}

func TestLegacyTerminalPolicies(t *testing.T) {
	rich := TerminalProfile{StdoutIsTTY: true, TerminalWidth: 120, ColorMode: "truecolor", Truecolor: true, Hyperlinks: true, Styling: true, DetectionSource: "terminfo"}
	for _, test := range []struct {
		name, format string
		profile      TerminalProfile
		plain        bool
	}{
		{"plain_policy_disables_all_output", "text", TerminalProfile{}, true},
		{"json_policy_from_profile_forces_plain", "json", rich, true},
		{"text_policy_preserves_rich_features", "text", rich, false},
		{"non_tty_profile_forces_plain_policy", "text", BuildTerminalProfile(CapabilityInput{Width: 200, Env: map[string]string{"TERM": "xterm-ghostty"}}), true},
	} {
		t.Run(test.name, func(t *testing.T) {
			got := RenderingPolicy(test.profile, test.format)
			if got.PlainOutput != test.plain || got.EmitANSIColor == test.plain || got.EmitTruecolor == test.plain || got.EmitHyperlinks == test.plain || !got.PreserveVisibleTargets {
				t.Fatalf("terminal policy = %+v", got)
			}
		})
	}
	t.Run("build_environment_summary_reads_env", func(t *testing.T) {
		if _, err := json.Marshal(TerminalDiagnostics(TerminalProfile{}, "text", nil).Environment); err != nil {
			t.Fatal(err)
		}
	})
}

func TestLegacyPreconfirmedOperations(t *testing.T) {
	for _, test := range []struct {
		name       string
		yes, force bool
	}{{"yes_flag_skips_prompt", true, false}, {"force_flag_skips_prompt", false, true}} {
		t.Run(test.name, func(t *testing.T) {
			args := ConfirmArgs{VerbLabel: "removal", Kind: "project", Identity: "demo", Yes: test.yes, Force: test.force}
			err := RequireConfirmation(args, io.Discard, func() (io.ReadCloser, error) { t.Fatal("preconfirmed operation opened terminal"); return nil, nil })
			if err != nil {
				t.Fatal(err)
			}
		})
	}
}
