package lifecycle

import (
	"errors"
	"strings"
	"testing"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
)

func legacyOwnershipInput(mode string, paths ...string) (model.MindConfig, []model.Mind, Snapshot) {
	config := model.DefaultMindConfig()
	config.ContentLayout = &mode
	minds := []model.Mind{{Name: "brief"}, {Name: "empty"}}
	files := Snapshot{}
	for _, path := range paths {
		parts := strings.Split(path, "/")
		for i := 1; i < len(parts); i++ {
			files[strings.Join(parts[:i], "/")] = Entry{Kind: "directory"}
		}
		files[path] = Entry{Kind: "file", Bytes: []byte("data")}
	}
	return config, minds, files
}

func TestLegacyOwnedInventory(t *testing.T) {
	t.Run("all_roles_outputs_and_additional_have_bidirectional_mapping", func(t *testing.T) {
		config, minds, files := legacyOwnershipInput("category",
			"outputs/brief/design.md", "outputs/brief/summary/01.md", "prompts/brief.md", "thinking/brief.md",
			"sources/brief/evidence.pdf", "assets/brief/img.png", "reviews/brief.md", "evals/brief/raw.json", "evals/brief/.keep")
		items, err := CollectOwnedRoots(config, minds, nil, files)
		if err != nil {
			t.Fatal(err)
		}
		if len(items) != 7 {
			t.Fatalf("owned roots = %#v, want 7", items)
		}
		forward := OwnedMapping(items, "bundle", map[string]string{"brief": "brief"})
		if forward["reviews/brief.md"] != "brief/reviews.md" || forward["prompts/brief.md"] != "brief/prompt.md" {
			t.Fatalf("bundle mapping = %#v", forward)
		}
		for _, item := range items {
			if got := OwnedDestination("category", item.Mind, item.Kind, item.File); got != item.Path {
				t.Errorf("category round trip = %q, want %q", got, item.Path)
			}
		}
	})
	t.Run("configured_legacy_core_roots_are_not_silently_skipped", func(t *testing.T) {
		for _, field := range []string{"articles", "sources", "assets"} {
			t.Run(field, func(t *testing.T) {
				config, minds, files := legacyOwnershipInput("category", "evidence/brief/note.md")
				custom := "evidence"
				switch field {
				case "articles":
					config.Layout.Articles = &custom
				case "sources":
					config.Layout.Sources = &custom
				case "assets":
					config.Layout.Assets = &custom
				}
				_, err := CollectOwnedRoots(config, minds, nil, files)
				var failure *output.Error
				if !errors.As(err, &failure) || failure.Kind != "ownership_unresolved" || !strings.Contains(err.Error(), "configured legacy core root") {
					t.Fatalf("configured %s accepted or misclassified: %v", field, err)
				}
			})
		}
	})
	t.Run("rejects_unknown_ownership_legacy_articles_and_shape_collisions", func(t *testing.T) {
		for _, test := range []struct {
			name, mode string
			paths      []string
		}{
			{"unknown-additional-owner", "category", []string{"reviews/unknown.md"}},
			{"loose-source", "category", []string{"sources/loose.pdf"}},
			{"legacy-article", "category", []string{"docs/brief.md"}},
			{"category-shapes", "category", []string{"reviews/brief.md", "reviews/brief/detail.md"}},
			{"bundle-shapes", "bundle", []string{"brief/reviews.md", "brief/reviews/detail.md"}},
			{"file-source-role", "bundle", []string{"brief/sources.md"}},
		} {
			t.Run(test.name, func(t *testing.T) {
				config, minds, files := legacyOwnershipInput(test.mode, test.paths...)
				if _, err := CollectOwnedRoots(config, minds, nil, files); err == nil {
					t.Fatalf("%s ownership/shape accepted: %v", test.mode, test.paths)
				}
			})
		}
	})
}
