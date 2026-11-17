package content

import (
	"reflect"
	"testing"

	"github.com/alswl/mind-forge/internal/model"
)

func TestLegacyPromptBindingResolution(t *testing.T) {
	for _, test := range []struct {
		name    string
		prompts []model.Prompt
		status  string
	}{
		{"resolve_prompt_bindings_bound", []model.Prompt{{Path: "prompts/my-post.md", Article: "docs/my-post.md"}}, "bound"},
		{"resolve_prompt_bindings_orphan_when_article_missing", []model.Prompt{{Path: "prompts/widowed.md", Article: "docs/does-not-exist.md"}}, "orphan"},
		{"resolve_prompt_bindings_orphan_when_article_empty", []model.Prompt{{Path: "prompts/unbound.md", Article: ""}}, "orphan"},
		{"resolve_prompt_bindings_duplicate_when_two_prompts_share_article", []model.Prompt{{Path: "prompts/my-post.md", Article: "docs/my-post.md"}, {Path: "prompts/my-post-old.md", Article: "docs/my-post.md"}}, "duplicate"},
	} {
		t.Run(test.name, func(t *testing.T) {
			articles := []model.Article{{Path: "docs/my-post.md"}}
			for i := range test.prompts {
				test.prompts[i].UpdatedAt = "2026-07-12T00:00:00Z"
			}
			got := resolvePromptBindings(model.Index{Articles: &articles, Prompts: &test.prompts})
			if len(got) != len(test.prompts) {
				t.Fatalf("binding count = %d, want %d", len(got), len(test.prompts))
			}
			for i, binding := range got {
				if binding.Status != test.status || !reflect.DeepEqual(binding.Prompt, test.prompts[i]) {
					t.Errorf("binding[%d] = %#v, want unchanged prompt %#v and status %q", i, binding, test.prompts[i], test.status)
				}
			}
		})
	}
}

func TestPromptBindingMissingArticlesAndEmptySections(t *testing.T) {
	if got := resolvePromptBindings(model.Index{}); len(got) != 0 {
		t.Fatalf("absent prompts produced bindings: %#v", got)
	}
	empty := []model.Prompt{}
	if got := resolvePromptBindings(model.Index{Prompts: &empty}); len(got) != 0 {
		t.Fatalf("empty prompts produced bindings: %#v", got)
	}
	prompts := []model.Prompt{{Path: "prompts/z.md", Article: "docs/missing.md"}, {Path: "prompts/a.md", Article: "docs/missing.md"}}
	got := resolvePromptBindings(model.Index{Prompts: &prompts})
	if len(got) != 2 {
		t.Fatalf("missing articles lost prompts: %#v", got)
	}
	for i, binding := range got {
		if binding.Status != "orphan" || binding.Prompt != prompts[i] {
			t.Errorf("missing-article duplicates must remain ordered orphans: %#v", got)
		}
	}
}
