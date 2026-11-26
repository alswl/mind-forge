package content

import (
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/alswl/mind-forge/pkg/dal/store"
	"github.com/stretchr/testify/require"
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

func TestLegacyArticleStatusRoundTripsThroughIndexRefresh(t *testing.T) {
	root := canonicalPath(t.TempDir())
	const source = "# Synthetic note\n"
	const index = "schema: '1'\narticles:\n  docs/example:\n    title: Synthetic note\n    article_path: docs/example.md\n    type: arch\n    created_at: original\n"
	putContent(t, root, "mind.yaml", "schema: '1'\n")
	putContent(t, root, "mind-index.yaml", index)
	putContent(t, root, "docs/example.md", source)
	putContent(t, root, "docs/unregistered.md", "# Unregistered\n")
	indexPath := filepath.Join(root, "mind-index.yaml")
	completed, invalid := "completed", "archived"
	_, err := UpdateArticle(root, "docs/example.md", ArticleUpdateOptions{Status: &completed, DryRun: true})
	require.NoError(t, err)
	_, err = UpdateArticle(root, "docs/example.md", ArticleUpdateOptions{Status: &invalid})
	require.ErrorContains(t, err, "invalid status")
	raw, err := os.ReadFile(indexPath)
	require.NoError(t, err)
	require.Equal(t, index, string(raw))

	config, err := ConfigFor(root)
	require.NoError(t, err)
	for _, status := range []string{"completed", "abandoned", "published", "draft"} {
		update, err := UpdateArticle(root, "docs/example.md", ArticleUpdateOptions{Status: &status})
		require.NoError(t, err)
		document, err := store.ReadIndex(indexPath, false)
		require.NoError(t, err)
		discovered, _, err := DiscoverArticles(root, config, time.Time{})
		require.NoError(t, err)
		require.NoError(t, document.Write(indexPath, discovered, false))
		shown, _, err := ShowArticle(root, "docs/example.md")
		require.NoError(t, err)
		require.Equal(t, update.Article, shown.Article)
		require.Equal(t, "original", shown.Article.CreatedAt)
		require.Equal(t, "arch", shown.Article.Type)
		result, err := ListArticlesByStatus(root, root, nil, &status, nil)
		require.NoError(t, err)
		if status == "draft" {
			require.Len(t, result.Items, 2)
		} else {
			require.Len(t, result.Items, 1)
			require.Equal(t, shown.Article, result.Items[0].Article)
		}
	}
	raw, err = os.ReadFile(filepath.Join(root, "docs/example.md"))
	require.NoError(t, err)
	require.Equal(t, source, string(raw))
}
