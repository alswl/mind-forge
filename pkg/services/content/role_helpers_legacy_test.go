package content

import (
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/alswl/mind-forge/pkg/dal/store"
)

func TestLegacyPromptFrontMatter(t *testing.T) {
	for _, test := range []struct{ name, body, article, mode string }{
		{"parses_article_and_mode", "---\narticle: docs/my-post.md\nmode: research\n---\n\nBody.\n", "docs/my-post.md", "research"},
		{"tolerates_missing_frontmatter", "Just prose, no frontmatter.\n", "", ""},
		{"tolerates_unclosed_frontmatter", "---\narticle: docs/my-post.md\n\nno closing delimiter\n", "docs/my-post.md", ""},
		{"tolerates_unknown_mode_value", "---\narticle: docs/my-post.md\nmode: not-a-real-mode\n---\n", "docs/my-post.md", ""},
	} {
		t.Run(test.name, func(t *testing.T) {
			article, mode := ParsePromptFrontMatter(test.body)
			if article != test.article {
				t.Fatalf("article = %q, want %q", article, test.article)
			}
			if test.mode == "" {
				if mode != nil {
					t.Fatalf("mode = %q, want absent", *mode)
				}
				return
			}
			if mode == nil || *mode != test.mode {
				t.Fatalf("mode = %v, want %q", mode, test.mode)
			}
		})
	}
}

func TestLegacyRoleReconcileTimes(t *testing.T) {
	for _, role := range []string{"prompt", "thinking"} {
		t.Run(role, func(t *testing.T) {
			project := t.TempDir()
			article := "docs/x.md"
			section, relative, body := "prompts", "prompts/my-prompt.md", "---\narticle: docs/x.md\n---\nBody.\n"
			if role == "thinking" {
				article = ""
				section, relative, body = "thinking", "thinking/notes.md", "Some thinking notes.\n"
			}
			putContent(t, project, relative, body)
			putContent(t, project, "mind-index.yaml", "schema: '1'\n"+section+":\n- path: "+relative+"\n  article: '"+article+"'\n  updated_at: '2020-01-01T00:00:00Z'\n")
			full := filepath.Join(project, relative)
			future := time.Now().Add(time.Hour)
			if err := os.Chtimes(full, future, future); err != nil {
				t.Fatal(err)
			}
			if role == "prompt" {
				if _, err := ReconcilePrompts(project, false); err != nil {
					t.Fatal(err)
				}
			} else {
				if _, err := ReconcileThinking(project, false); err != nil {
					t.Fatal(err)
				}
			}
			document, err := store.ReadIndex(filepath.Join(project, "mind-index.yaml"), false)
			if err != nil {
				t.Fatal(err)
			}
			if role == "prompt" {
				if document.Value.Prompts == nil || len(*document.Value.Prompts) != 1 {
					t.Fatalf("prompts = %+v", document.Value.Prompts)
				}
				if got := (*document.Value.Prompts)[0].UpdatedAt; got != "2020-01-01T00:00:00Z" {
					t.Fatalf("updated_at = %q", got)
				}
			} else {
				if document.Value.Thinking == nil || len(*document.Value.Thinking) != 1 {
					t.Fatalf("thinking = %+v", document.Value.Thinking)
				}
				if got := (*document.Value.Thinking)[0].UpdatedAt; got != "2020-01-01T00:00:00Z" {
					t.Fatalf("updated_at = %q", got)
				}
			}
			after, err := os.ReadFile(full)
			if err != nil || string(after) != body {
				t.Fatalf("author body changed: %q, %v", after, err)
			}
		})
	}
}
