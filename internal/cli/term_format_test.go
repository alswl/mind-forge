package cli

import (
	"strings"
	"testing"

	"github.com/alswl/mind-forge/internal/model"
)

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
