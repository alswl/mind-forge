package output

import (
	"encoding/json"
	"reflect"
	"testing"
)

func TestTerminalDiagnosticReport(t *testing.T) {
	profile := BuildTerminalProfile(CapabilityInput{Width: 80, Env: map[string]string{"TERM": "dumb"}})
	report := TerminalDiagnostics(profile, "json", map[string]string{"TERM": "dumb", "COLORTERM": ""})
	if report.Policy.EmitANSIColor || report.Policy.EmitHyperlinks || len(report.Checks) != 3 || report.Checks[0].Status != "fail" {
		t.Fatalf("pipe diagnostics: %#v", report)
	}
	if !reflect.DeepEqual(report.Recommendations, []string{"Fallback active: stdout is not a terminal", "Run with MF_FORCE_TTY=1 to test terminal behavior in pipes"}) {
		t.Fatal(report.Recommendations)
	}
	if report.Environment.Colorterm == nil || *report.Environment.Colorterm != "" || report.Environment.TermProgram != nil {
		t.Fatal("missing and empty environment values were conflated")
	}
	if report.Text() != "Terminal: dumb\nTTY: no\nColor: none\nHyperlinks: no\nFallback: stdout is not a terminal\n" {
		t.Fatal(report.Text())
	}
	profile = BuildTerminalProfile(CapabilityInput{StdoutIsTTY: true, Width: 100, Env: map[string]string{"COLORTERM": "truecolor", "MF_FORCE_HYPERLINKS": "1"}})
	report = TerminalDiagnostics(profile, "text", nil)
	raw, err := json.Marshal(report)
	if err != nil {
		t.Fatal(err)
	}
	var fields map[string]json.RawMessage
	if err := json.Unmarshal(raw, &fields); err != nil || string(fields["recommendations"]) != "[]" || report.Checks[1].Status != "pass" || report.Checks[2].Status != "pass" {
		t.Fatalf("capable terminal diagnostics: %s (%v)", raw, err)
	}
}
