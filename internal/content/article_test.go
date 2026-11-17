package content

import (
	"strings"
	"testing"
)

func TestArticleSelectorMatchesExactFormsAndRejectsAmbiguity(t *testing.T) {
	root := canonicalPath(t.TempDir())
	putContent(t, root, "mind-index.yaml", "schema: '1'\narticles:\n  docs/weekly:\n    title: Weekly Report\n    article_path: docs/weekly/\n  outputs/other:\n    title: Generated\n    article_path: outputs/other.md\n")
	for _, selector := range []string{"docs/weekly/", "docs/weekly", "weekly", "Weekly Report"} {
		got, err := ResolveArticleSelector(root, selector)
		if err != nil || got != "docs/weekly/" {
			t.Errorf("%s: %s (%v)", selector, got, err)
		}
	}
	if _, err := ResolveArticleSelector(root, "week"); err == nil {
		t.Fatal("substring accepted")
	}
	putContent(t, root, "mind-index.yaml", "schema: '1'\narticles:\n  docs/dup:\n    title: Authored\n    article_path: docs/dup\n  outputs/dup:\n    title: Generated\n    article_path: outputs/dup.md\n")
	if _, err := ResolveArticleSelector(root, "dup"); err == nil || !strings.Contains(err.Error(), "ambiguous") {
		t.Fatal("ambiguity not rejected", err)
	}
}
