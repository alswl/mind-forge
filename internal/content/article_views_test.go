package content

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/alswl/mind-forge/internal/model"
)

func TestArticleBindingsUseIndexAuthorityAndStableDuplicatePrimary(t *testing.T) {
	mode := "editorial"
	articles := []model.Article{{Path: "docs/draft.md"}}
	prompts := []model.Prompt{{Path: "prompts/z.md", Article: "docs/draft.md", Mode: &mode}, {Path: "prompts/a.md", Article: "docs/draft.md"}, {Path: "prompts/orphan.md", Article: "docs/missing.md"}}
	thoughts := []model.Thinking{{Path: "thinking/draft.md", Article: "wrong-binding", UpdatedAt: "time"}}
	byPath, byKey := articleBindings(model.Index{Articles: &articles, Prompts: &prompts, Thinking: &thoughts})
	view := byPath["docs/draft.md"]
	if view == nil || view.Path != "prompts/a.md" || view.BindingStatus != "duplicate" || strings.Join(view.Conflicts, ",") != "prompts/a.md,prompts/z.md" {
		t.Fatalf("duplicates %#v", view)
	}
	if orphan := byPath["docs/missing.md"]; orphan == nil || orphan.BindingStatus != "orphan" {
		t.Fatalf("orphan %#v", orphan)
	}
	if thought := byKey["draft"]; thought == nil || thought.Path != "thinking/draft.md" {
		t.Fatalf("thinking %#v", thought)
	}
}

func TestArticleReadViewsAreReadOnlyAndRetainLegacyShowSelection(t *testing.T) {
	root := canonicalPath(t.TempDir())
	putContent(t, root, "mind.yaml", "schema: '1'\n")
	putContent(t, root, "mind-index.yaml", "schema: '1'\narticles:\n  docs/monthly:\n    title: Monthly Title\n    article_path: docs/monthly.md\n")
	putContent(t, root, "docs/monthly.md", "# Body title\n")
	before, _ := os.ReadFile(filepath.Join(root, "mind-index.yaml"))
	items, _, err := ListArticleItems(root)
	if err != nil || len(items) != 1 {
		t.Fatalf("list %#v %v", items, err)
	}
	item := items[0]
	if item.Data["content_kind"] != "single_file" || item.Data["identity"] != "docs/monthly.md" || item.Data["article_dir"] != "docs/monthly" || item.Mtime == 0 {
		t.Fatalf("list item %#v", item)
	}
	shown, _, err := ShowArticle(root, "month")
	if err != nil || shown.Article.Title != "Monthly Title" {
		t.Fatalf("legacy substring show %#v %v", shown, err)
	}
	if _, err := ResolveArticleSelector(root, "month"); err == nil {
		t.Fatal("write selector inherited loose read matching")
	}
	if after, _ := os.ReadFile(filepath.Join(root, "mind-index.yaml")); string(after) != string(before) {
		t.Fatal("read views wrote index")
	}
	if _, _, err := ShowArticle(root, "../outside"); err == nil {
		t.Fatal("outside selector accepted")
	}
}

func TestFrozenArticleBindings(t *testing.T) {
	t.Run("prompt_view_none_when_no_prompt_binds", func(t *testing.T) {
		articles := []model.Article{{Path: "docs/a.md"}}
		prompts, _ := articleBindings(model.Index{Articles: &articles})
		if prompts["docs/a.md"] != nil {
			t.Fatal("invented prompt")
		}
	})
	t.Run("prompt_view_bound_single_match", func(t *testing.T) {
		articles := []model.Article{{Path: "docs/a.md"}}
		mode := "research"
		entries := []model.Prompt{{Path: "prompts/a.md", Article: "docs/a.md", Mode: &mode, UpdatedAt: "2026-07-12T00:00:00Z"}}
		prompts, _ := articleBindings(model.Index{Articles: &articles, Prompts: &entries})
		view := prompts["docs/a.md"]
		if view == nil || view.Path != "prompts/a.md" || view.BindingStatus != "bound" || len(view.Conflicts) != 0 {
			t.Fatalf("%#v", view)
		}
	})
	t.Run("prompt_view_duplicate_lists_all_conflicts", func(t *testing.T) {
		articles := []model.Article{{Path: "docs/a.md"}}
		entries := []model.Prompt{{Path: "prompts/b.md", Article: "docs/a.md"}, {Path: "prompts/a.md", Article: "docs/a.md"}}
		prompts, _ := articleBindings(model.Index{Articles: &articles, Prompts: &entries})
		view := prompts["docs/a.md"]
		if view == nil || view.BindingStatus != "duplicate" || strings.Join(view.Conflicts, ",") != "prompts/a.md,prompts/b.md" {
			t.Fatalf("%#v", view)
		}
	})
	t.Run("thinking_view_present_and_absent", func(t *testing.T) {
		articles := []model.Article{{Path: "docs/a.md"}}
		entries := []model.Thinking{{Path: "thinking/a.md", Article: "docs/a.md"}}
		_, thoughts := articleBindings(model.Index{Articles: &articles, Thinking: &entries})
		if thoughts[ArticleOutputStem("docs/a.md")] == nil || thoughts[ArticleOutputStem("docs/missing.md")] != nil {
			t.Fatal("thinking lookup mismatch")
		}
	})
	t.Run("batch_prompt_views_match_per_article_lookup", func(t *testing.T) {
		articles := []model.Article{{Path: "docs/a.md"}, {Path: "docs/b.md"}}
		mode := "research"
		entries := []model.Prompt{{Path: "prompts/a.md", Article: "docs/a.md", Mode: &mode}, {Path: "prompts/b.md", Article: "docs/b.md"}, {Path: "prompts/b-dup.md", Article: "docs/b.md"}}
		prompts, _ := articleBindings(model.Index{Articles: &articles, Prompts: &entries})
		if len(prompts) != 2 || prompts["docs/a.md"].BindingStatus != "bound" || prompts["docs/a.md"].Path != "prompts/a.md" || prompts["docs/b.md"].BindingStatus != "duplicate" || strings.Join(prompts["docs/b.md"].Conflicts, ",") != "prompts/b-dup.md,prompts/b.md" {
			t.Fatalf("%#v", prompts)
		}
	})
	t.Run("batch_thinking_views_match_per_article_lookup", func(t *testing.T) {
		articles := []model.Article{{Path: "docs/a.md"}}
		entries := []model.Thinking{{Path: "thinking/a.md", Article: "docs/a.md"}}
		_, thoughts := articleBindings(model.Index{Articles: &articles, Thinking: &entries})
		if len(thoughts) != 1 || thoughts[ArticleOutputStem("docs/a.md")].Path != "thinking/a.md" {
			t.Fatalf("%#v", thoughts)
		}
	})
}
