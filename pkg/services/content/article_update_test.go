package content

import (
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestArticleUpdatePreviewAndAssociatedPrompt(t *testing.T) {
	root := canonicalPath(t.TempDir())
	putContent(t, root, "mind.yaml", "schema: '1'\n")
	putContent(t, root, "mind-index.yaml", "schema: '1'\narticles:\n  docs/draft:\n    title: Old\n    article_path: docs/draft.md\n    updated_at: previous\n")
	putContent(t, root, "docs/draft.md", "# Author title\n")
	putContent(t, root, "prompts/draft.md", "---\ntitle: Old\narticle: docs/draft.md\n---\nPrompt body\n")
	title, status := "  New  ", "published"
	before, _ := os.ReadFile(filepath.Join(root, "mind-index.yaml"))
	preview, err := UpdateArticle(root, "Old", ArticleUpdateOptions{Title: &title, Status: &status, DryRun: true})
	if err != nil || preview.Article.Title != "Old" || preview.Article.Status != "draft" {
		t.Fatalf("preview %#v %v", preview, err)
	}
	if after, _ := os.ReadFile(filepath.Join(root, "mind-index.yaml")); string(after) != string(before) {
		t.Fatal("preview wrote index")
	}
	now := time.Date(2026, 2, 3, 4, 5, 6, 0, time.UTC)
	report, err := UpdateArticle(root, "docs/draft.md", ArticleUpdateOptions{Title: &title, Status: &status, Now: now})
	if err != nil || report.Article.Title != "New" || report.Article.Status != "published" || report.Article.UpdatedAt != "2026-02-03T04:05:06Z" {
		t.Fatalf("update %#v %v", report, err)
	}
	if raw, _ := os.ReadFile(filepath.Join(root, "docs/draft.md")); string(raw) != "# Author title\n" {
		t.Fatal("author body changed")
	}
	if raw, _ := os.ReadFile(filepath.Join(root, "prompts/draft.md")); string(raw) != "---\ntitle: New\narticle: docs/draft.md\n---\nPrompt body\n" {
		t.Fatalf("prompt %q", raw)
	}
	if _, err := UpdateArticle(root, "New", ArticleUpdateOptions{}); err == nil {
		t.Fatal("empty update accepted")
	}
	empty := " \n"
	if _, err := UpdateArticle(root, "New", ArticleUpdateOptions{Title: &empty}); err == nil {
		t.Fatal("empty title accepted")
	}
}
