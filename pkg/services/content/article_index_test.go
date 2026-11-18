package content

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/alswl/mind-forge/pkg/common/model"
	"github.com/alswl/mind-forge/pkg/dal/store"
)

func TestArticleDiscoveryPreservesMetadataAndDeclaredPrecedence(t *testing.T) {
	root := canonicalPath(t.TempDir())
	putContent(t, root, "mind.yaml", "schema: '1'\nbuild:\n  articles:\n    configured:\n      article_dir: drafts/shared\n    missing: {}\narticles:\n  configured: {article_dir: ignored}\n  compat: {article_dir: drafts/compat}\ntemplates:\n  daily:\n    pattern: outputs/{date:YYYY-MM}/{date:YYYY-MM-DD}.md\n    mode: generated\n")
	putContent(t, root, "mind-index.yaml", "schema: '1'\narticles:\n  docs/draft:\n    title: Existing title\n    article_path: docs/draft/\n    created_at: old-created\n    updated_at: old-updated\n  docs/index-only:\n    title: Kept\n    article_path: docs/index-only.md\n")
	putContent(t, root, "docs/draft/01-opening.md", "# Unstable title\n")
	putContent(t, root, "docs/fresh-file.md", "# Ignore H1\n")
	putContent(t, root, "drafts/shared/01-first.md", "one")
	putContent(t, root, "drafts/shared/02-second.md", "two")
	putContent(t, root, "drafts/compat/compat.md", "compat")
	putContent(t, root, "outputs/2026-02/2026-02-03.md", "generated")
	config, err := ConfigFor(root)
	if err != nil {
		t.Fatal(err)
	}
	first, warnings, err := DiscoverArticles(root, config, time.Date(2026, 2, 3, 4, 5, 6, 0, time.UTC))
	if err != nil {
		t.Fatal(err)
	}
	if len(warnings) != 1 {
		t.Fatalf("warnings %#v", warnings)
	}
	byPath := map[string]model.Article{}
	for _, article := range *first.Articles {
		byPath[article.Path] = article
	}
	if len(byPath) != 7 {
		t.Fatalf("discovery %#v", byPath)
	}
	if got := byPath["docs/draft"]; got.Title != "Existing title" || got.CreatedAt != "old-created" || got.UpdatedAt != "old-updated" {
		t.Fatalf("metadata %#v", got)
	}
	if got := byPath["docs/fresh-file.md"]; got.Title != "fresh file" {
		t.Fatalf("title %#v", got)
	}
	if got := byPath["drafts/shared"]; got.Title != "configured" {
		t.Fatalf("declared source %#v", got)
	}
	generated := byPath["outputs/2026-02/2026-02-03.md"]
	if generated.TemplateOrigin == nil || generated.TemplateOrigin.SlotValue != "2026-02-03" {
		t.Fatalf("generated %#v", generated)
	}
	if _, ok := byPath["docs/index-only.md"]; !ok {
		t.Fatal("index-only registration lost")
	}
	second, _, err := DiscoverArticles(root, config, time.Date(2026, 3, 3, 4, 5, 6, 0, time.UTC))
	if err != nil {
		t.Fatal(err)
	}
	for _, article := range *second.Articles {
		if article.Path == generated.Path && article.CreatedAt != generated.CreatedAt {
			t.Fatal("unindexed generated timestamp changed")
		}
	}
	if raw, _ := os.ReadFile(filepath.Join(root, "mind-index.yaml")); !strings.Contains(string(raw), "article_path: docs/draft/") {
		t.Fatal("discovery wrote index")
	}
}

func TestGeneratedArticlePatternValidatesAndMatchesReferenceSlots(t *testing.T) {
	root := canonicalPath(t.TempDir())
	putContent(t, root, "mind.yaml", "schema: '1'\n")
	putContent(t, root, "outputs/----/----.--.md", "text")
	config, err := ConfigFor(root)
	if err != nil {
		t.Fatal(err)
	}
	templates := map[string]model.Template{"bad": {Pattern: "out/{date:YYYY}/{date:YYYY}.md", Mode: "generated"}}
	config.Templates = &templates
	if _, err := ScanTemplateArticles(root, config); err == nil {
		t.Fatal("duplicate date slot accepted")
	}
	templates["bad"] = model.Template{Pattern: "out/{unknown:X}.md", Mode: "generated"}
	if _, err := ScanTemplateArticles(root, config); err == nil {
		t.Fatal("unknown placeholder accepted")
	}
}

func TestArticleIndexDiffPreviewAndRoleReconcile(t *testing.T) {
	root := canonicalPath(t.TempDir())
	putContent(t, root, "mind.yaml", "schema: '1'\n")
	original := "schema: '1'\narticles:\n  docs/gone:\n    title: Gone\n    article_path: docs/gone.md\nopaque: [keep] # literal\n"
	putContent(t, root, "mind-index.yaml", original)
	putContent(t, root, "docs/new-post/01-opening.md", "# Title\n")
	putContent(t, root, "thinking/new-post.md", "thoughts")
	preview, _, err := IndexArticles(root, true, time.Time{})
	if err != nil {
		t.Fatal(err)
	}
	if len(preview.Details["added"].([]model.ScannedArticle)) != 1 || len(preview.Details["removed"].([]model.Article)) != 1 {
		t.Fatalf("preview %#v", preview)
	}
	if raw, _ := os.ReadFile(filepath.Join(root, "mind-index.yaml")); string(raw) != original {
		t.Fatal("preview wrote index")
	}
	if _, _, err := IndexArticles(root, false, time.Time{}); err != nil {
		t.Fatal(err)
	}
	index, err := store.ReadIndex(filepath.Join(root, "mind-index.yaml"), false)
	if err != nil {
		t.Fatal(err)
	}
	if len(*index.Value.Articles) != 1 || (*index.Value.Articles)[0].Path != "docs/new-post" {
		t.Fatalf("articles %#v", index.Value.Articles)
	}
	if (*index.Value.Thinking)[0].Article != "docs/new-post" {
		t.Fatal("thinking resolved before article publication")
	}
	if raw, _ := os.ReadFile(filepath.Join(root, "mind-index.yaml")); !strings.Contains(string(raw), "opaque: [keep] # literal") {
		t.Fatal("opaque metadata lost")
	}
}

func TestArticleDiffDoesNotReplaceTrailingSlashRegistration(t *testing.T) {
	path := "docs/monthly"
	articles := []model.Article{{Title: "Original", Path: path + "/", CreatedAt: "first", UpdatedAt: "last"}}
	scanned := []model.ScannedArticle{{Title: "monthly", Filename: "monthly", ArticleDir: &path, ArticlePath: &path}}
	diff := ArticleDiff(model.Index{Articles: &articles}, scanned, "docs")
	if len(diff.Added) != 0 || len(diff.Removed) != 0 {
		t.Fatalf("same article misclassified %#v", diff)
	}
}
