package content

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestArticleReadViewsRetainIndexWarningsAndLastValue(t *testing.T) {
	for _, test := range []struct{ name, raw, want, title string }{
		{"top-level", "schema: '1'\narticles:\n  first: {title: First, article_path: docs/first.md}\narticles:\n  second: {title: Second, article_path: docs/second.md}\n", "duplicate top-level key", "Second"},
		{"nested", "schema: '1'\narticles:\n  second:\n    title: First\n    title: Second\n    article_path: docs/second.md\n", "duplicate key \"title\"", "Second"},
		{"valid", "schema: '1'\narticles:\n  second: {title: Second, article_path: docs/second.md}\n", "", "Second"},
	} {
		t.Run(test.name, func(t *testing.T) {
			root := canonicalPath(t.TempDir())
			putContent(t, root, "mind.yaml", "schema: '1'\n")
			putContent(t, root, "mind-index.yaml", test.raw)
			rows, warnings, err := ListArticleItems(root)
			if err != nil || len(rows) != 1 || rows[0].Article.Title != test.title {
				t.Fatalf("rows=%+v warnings=%v err=%v", rows, warnings, err)
			}
			if test.want == "" {
				if len(warnings) != 0 {
					t.Fatalf("unexpected warnings: %v", warnings)
				}
			} else if len(warnings) != 1 || !strings.Contains(warnings[0], test.want) {
				t.Fatalf("want %q, got %v", test.want, warnings)
			}
			if test.name == "top-level" && !strings.Contains(warnings[0], "articles") {
				t.Fatalf("key name lost: %v", warnings)
			}
			if test.name == "nested" && strings.Contains(warnings[0], "top-level") {
				t.Fatalf("nested key misclassified: %v", warnings)
			}
			shown, showWarnings, err := ShowArticle(root, "docs/second.md")
			if err != nil || shown.Article.Title != test.title || strings.Join(showWarnings, "\n") != strings.Join(warnings, "\n") {
				t.Fatalf("show=%+v warnings=%v err=%v", shown, showWarnings, err)
			}
			after, err := os.ReadFile(filepath.Join(root, "mind-index.yaml"))
			if err != nil || string(after) != test.raw {
				t.Fatalf("read changed index: %q %v", after, err)
			}
		})
	}
}
