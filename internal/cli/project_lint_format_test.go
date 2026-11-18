package cli

import (
	"github.com/alswl/mind-forge/internal/content"
	"github.com/alswl/mind-forge/internal/output"
	"strings"
	"testing"
)

func TestProjectLintTextRetainsFindingsAndSummary(t *testing.T) {
	issues := []map[string]any{{"severity": "warning", "kind": "duplicate_key", "message": "duplicate key 'articles' at line 5", "path": "mind-index.yaml"}}
	result := projectLintResult(issues, content.ProjectLintSummary{Warnings: 1, Fixed: 2}, false)
	text := output.RenderVerb(result, output.DisplayOptions{})
	for _, want := range []string{"[warning] duplicate_key: duplicate key 'articles' at line 5 (mind-index.yaml)", "0 errors, 1 warnings, 0 info, 2 fixed"} {
		if !strings.Contains(text, want) {
			t.Errorf("missing %q in %q", want, text)
		}
	}
	empty := output.RenderVerb(projectLintResult(nil, content.ProjectLintSummary{}, true), output.DisplayOptions{})
	if !strings.Contains(empty, "0 errors, 0 warnings, 0 info") {
		t.Fatalf("empty summary lost: %q", empty)
	}
}
