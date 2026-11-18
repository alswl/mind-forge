package content

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/alswl/mind-forge/pkg/dal/store"
)

func TestRoleIndexReparsesBindingsAndPreservesExistingTimes(t *testing.T) {
	root := canonicalPath(t.TempDir())
	before := "schema: '1'\narticles:\n  docs/draft:\n    title: Draft\n    article_path: docs/draft.md\nprompts:\n  draft: {path: prompts/draft.md, article: old, updated_at: old-time}\n  missing: {path: prompts/missing.md, article: '', updated_at: previous}\nopaque: {keep: literal}\n"
	putContent(t, root, "mind-index.yaml", before)
	putContent(t, root, "prompts/draft.md", "---\narticle: 'docs/draft.md'\nmode: research\n")
	putContent(t, root, "thinking/draft.md", "no frontmatter required")
	preview, err := ReconcilePrompts(root, true)
	if err != nil || preview.KeptCount != 1 || len(preview.Removed) != 1 {
		t.Fatalf("preview %#v %v", preview, err)
	}
	if raw, _ := os.ReadFile(filepath.Join(root, "mind-index.yaml")); string(raw) != before {
		t.Fatal("preview wrote index")
	}
	if _, err := ReconcilePrompts(root, false); err != nil {
		t.Fatal(err)
	}
	if _, err := ReconcileThinking(root, false); err != nil {
		t.Fatal(err)
	}
	index, err := store.ReadIndex(filepath.Join(root, "mind-index.yaml"), false)
	if err != nil {
		t.Fatal(err)
	}
	prompt := (*index.Value.Prompts)[0]
	if prompt.Article != "docs/draft.md" || prompt.Mode == nil || *prompt.Mode != "research" || prompt.UpdatedAt != "old-time" {
		t.Fatalf("prompt %#v", prompt)
	}
	if entry := (*index.Value.Thinking)[0]; entry.Article != "docs/draft.md" {
		t.Fatalf("thinking %#v", entry)
	}
}

func TestPromptFrontMatterIsTolerantAndDoesNotInterpretMarkdownBody(t *testing.T) {
	cases := []struct{ body, article, mode string }{
		{"no frontmatter\narticle: docs/body.md\n", "", ""},
		{"---\narticle: 'docs/draft.md'\nmode: ' research '\n---\narticle: docs/body.md\n", "docs/draft.md", "research"},
		{"---\r\narticle: docs/open.md\r\nmode: unknown\r\n", "docs/open.md", ""},
		{"--- \narticle: docs/not-front.md\n", "", ""},
		{"---\n article: docs/indented.md\n---\n", "", ""},
	}
	for _, c := range cases {
		article, mode := ParsePromptFrontMatter(c.body)
		got := ""
		if mode != nil {
			got = *mode
		}
		if article != c.article || got != c.mode {
			t.Errorf("%q: %q %q", c.body, article, got)
		}
	}
}

func TestRoleProjectionsUseOnlyStoredIndexAndStablePathOrder(t *testing.T) {
	root := canonicalPath(t.TempDir())
	index := "schema: '1'\nprompts:\n  z: {path: prompts/z.md, article: docs/z.md, updated_at: z-time}\n  a: {path: prompts/a.md, article: docs/a.md, mode: research, updated_at: a-time}\nthinking:\n  z: {path: thinking/z.md, article: docs/z.md, updated_at: z-time}\n  a: {path: thinking/a.md, article: docs/a.md, updated_at: a-time}\n"
	putContent(t, root, "mind-index.yaml", index)
	putContent(t, root, "prompts/unindexed.md", "---\narticle: docs/new.md\n---\n")
	prompts, err := PromptProjection(root)
	if err != nil || len(prompts) != 2 || prompts[0].Path != "prompts/a.md" || prompts[1].Path != "prompts/z.md" {
		t.Fatalf("prompts %#v %v", prompts, err)
	}
	thinking, err := ThinkingProjection(root)
	if err != nil || len(thinking) != 2 || thinking[0].Path != "thinking/a.md" {
		t.Fatalf("thinking %#v %v", thinking, err)
	}
	if raw, _ := os.ReadFile(filepath.Join(root, "mind-index.yaml")); string(raw) != index {
		t.Fatal("projection read wrote index")
	}
	empty := canonicalPath(t.TempDir())
	if items, err := PromptProjection(empty); err != nil || items == nil || len(items) != 0 {
		t.Fatalf("empty %#v %v", items, err)
	}
	if _, err := os.Stat(filepath.Join(empty, "mind-index.yaml")); !os.IsNotExist(err) {
		t.Fatal("projection read created index")
	}
}

func TestRoleReconcileDoesNotMaterializeOptionalDirectories(t *testing.T) {
	root := canonicalPath(t.TempDir())
	putContent(t, root, "mind-index.yaml", "schema: '1'\n")
	for _, dryRun := range []bool{true, false} {
		if _, err := ReconcilePrompts(root, dryRun); err != nil {
			t.Fatal(err)
		}
		if _, err := ReconcileThinking(root, dryRun); err != nil {
			t.Fatal(err)
		}
		for _, dir := range []string{"prompts", "thinking"} {
			if _, err := os.Stat(filepath.Join(root, dir)); !os.IsNotExist(err) {
				t.Fatalf("optional %s materialized: %v", dir, err)
			}
		}
	}
}
