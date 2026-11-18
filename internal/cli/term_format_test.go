package cli

import (
	"bytes"
	"encoding/json"
	"strings"
	"testing"

	"github.com/alswl/mind-forge/internal/app"
	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
)

func TestTermWarningsRetainPrefixAndJSONPayload(t *testing.T) {
	for _, format := range []string{"text", "json"} {
		for _, quiet := range []bool{false, true} {
			var stdout, stderr bytes.Buffer
			context := &app.Context{Format: format, Quiet: quiet}
			warnings := []string{"original 'variant' overlaps term 'Synthetic'"}
			if err := renderCorrection(context, &stdout, &stderr, map[string]any{"created": true}, "added", warnings); err != nil {
				t.Fatal(err)
			}
			if got := stderr.String(); got != "warning: "+warnings[0]+"\n" {
				t.Errorf("format=%s quiet=%v warning=%q", format, quiet, got)
			}
			if quiet && stdout.Len() != 0 {
				t.Fatal("quiet leaked success output")
			}
			if format == "json" && !quiet {
				var body struct {
					Data struct {
						Created  bool
						Warnings []string
					}
					Warnings []string
				}
				if err := json.Unmarshal(stdout.Bytes(), &body); err != nil || !body.Data.Created || len(body.Data.Warnings) != 1 || body.Data.Warnings[0] != warnings[0] || body.Warnings != nil {
					t.Fatalf("warning envelope changed: %s (%v)", stdout.String(), err)
				}
			}
			stdout.Reset()
			stderr.Reset()
			result := output.VerbResult{Verb: output.Update, Kind: "term", Identity: "Synthetic", Details: map[string]any{}}
			if err := termVerb(context, &stdout, &stderr, result, []string{"scope fallback"}); err != nil {
				t.Fatal(err)
			}
			if stderr.String() != "WARN: scope fallback\n" {
				t.Fatalf("scope warning changed: %q", stderr.String())
			}
		}
	}
}

func TestSharedRendererPreservesBuildWarningPrefix(t *testing.T) {
	var stdout, stderr bytes.Buffer
	if err := renderTerm(&app.Context{Format: "text"}, &stdout, &stderr, nil, "built", []string{"merged private callout"}); err != nil {
		t.Fatal(err)
	}
	if stderr.String() != "WARN: merged private callout\n" {
		t.Fatalf("shared warning = %q", stderr.String())
	}
}

func TestTermFormatterDisclosesProtectedAndHeldBackSeparately(t *testing.T) {
	r := model.TermLintReport{Findings: []model.TermFinding{{Path: "a.md", Original: "机器", Correct: "装置", Term: "装置", HeldBack: true, Boundary: "standalone"}, {Path: "a.md", Original: "api", Correct: "API", Term: "API", QuoteProtected: true, Boundary: "standalone"}}, HeldBackCount: 1, QuoteProtectedCount: 1, ScannedFiles: 1}
	text := formatTermLint(r, true, false, "file", "docs/a.md")
	for _, part := range []string{"0 fixed", "1 finding held back", "mf term fix docs/a.md --term 装置", "1 finding inside quotes/blockquotes left untouched", "--include-quotes"} {
		if !strings.Contains(text, part) {
			t.Errorf("missing %q in %s", part, text)
		}
	}
	text = formatTermLint(r, false, false, "file", "docs/a.md")
	if !strings.Contains(text, ", held back") || !strings.Contains(text, ", quote-protected") {
		t.Fatalf("finding reasons lost: %s", text)
	}
}

func TestTermFormatterPreviewRetainsFindingProvenance(t *testing.T) {
	count := uint64(0)
	r := model.TermLintReport{Findings: []model.TermFinding{{Path: "synthetic.md", Line: 2, Column: 4, Original: "old", Correct: "new", Term: "Synthetic", MatchKind: "pinyin", FixKind: "suggested", Selection: "suggested_disabled", Context: "before old after"}}, WouldFixCount: &count, IneligibleCount: 1}
	text := formatTermLint(r, true, true, "file", "synthetic.md")
	for _, part := range []string{"target: file (dry-run preview)", "synthetic.md:2:4:", "match=pinyin confidence=none selection=suggested_disabled", "before old after", "selected 0"} {
		if !strings.Contains(text, part) {
			t.Errorf("missing %q in %s", part, text)
		}
	}
}

func TestWarningRendererPreservesPayloadNumbersAndCallerData(t *testing.T) {
	for _, enveloped := range []bool{false, true} {
		original := map[string]any{"size_bytes": json.Number("9007199254740993"), "created": true}
		var payload any = original
		if enveloped {
			payload = map[string]any{"status": "ok", "command": "mf", "data": original}
		}
		var out, errout bytes.Buffer
		if err := renderTerm(&app.Context{Format: "json"}, &out, &errout, payload, "", []string{"notice"}); err != nil {
			t.Fatal(err)
		}
		if !strings.Contains(out.String(), `"size_bytes":9007199254740993`) || !strings.Contains(out.String(), `"warnings":["notice"]`) {
			t.Fatal(out.String())
		}
		var body map[string]json.RawMessage
		if err := json.Unmarshal(out.Bytes(), &body); err != nil {
			t.Fatal(err)
		}
		if body["warnings"] != nil {
			t.Fatalf("warning at top level: %s", out.String())
		}
		if _, mutated := original["warnings"]; mutated {
			t.Fatal("renderer mutated caller data")
		}
	}
}

func TestEmptyGlossaryDisclosureDoesNotMasqueradeAsCleanContent(t *testing.T) {
	for _, fix := range []bool{false, true} {
		for _, dry := range []bool{false, true} {
			empty := model.TermLintReport{NoTermsRegistered: true, ScannedFiles: 1}
			if got := formatTermLint(empty, fix, dry, "project", ""); got != "No terms registered." {
				t.Fatalf("empty fix=%v dry=%v: %s", fix, dry, got)
			}
			clean := model.TermLintReport{ScannedFiles: 1}
			if got := formatTermLint(clean, fix, dry, "project", ""); got != "No term issues found." {
				t.Fatalf("clean fix=%v dry=%v: %s", fix, dry, got)
			}
		}
	}
	if got := formatTermLint(model.TermLintReport{}, false, false, "project", ""); got != "No term issues found." {
		t.Fatalf("registered glossary without files: %s", got)
	}
	failed := model.TermLintReport{NoTermsRegistered: true, Failures: []model.TermLintFailure{{Path: "broken.md", Reason: "invalid encoding"}}}
	if got := formatTermLint(failed, true, false, "project", ""); !strings.Contains(got, "broken.md") || strings.Contains(got, "No terms registered.") {
		t.Fatalf("registration disclosure hid failure: %s", got)
	}
	ordinary := model.TermLintReport{ScannedFiles: 1}
	empty := ordinary
	empty.NoTermsRegistered = true
	a, err := json.Marshal(ordinary)
	if err != nil {
		t.Fatal(err)
	}
	b, err := json.Marshal(empty)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(a, b) {
		t.Fatalf("registration disclosure changed JSON: %s != %s", a, b)
	}
}
