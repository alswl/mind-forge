package store

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/alswl/mind-forge/pkg/common/model"
	"go.yaml.in/yaml/v3"
)

const frozenDuplicateArticles = "schema: '1'\narticles:\n  first:\n    title: First\n    article_path: docs/first.md\narticles:\n  second:\n    title: Second\n    article_path: docs/second.md\n"

func TestLegacyIndexDuplicateHandling(t *testing.T) {
	t.Run("dedup_removes_duplicate_top_level_keys", func(t *testing.T) {
		document, err := ParseYAML([]byte(frozenDuplicateArticles))
		if err != nil {
			t.Fatal(err)
		}
		warnings := []string{}
		deduplicateYAML(&document.Root, &warnings, map[*yaml.Node]bool{})
		raw, err := yaml.Marshal(&document.Root)
		if err != nil {
			t.Fatal(err)
		}
		var tree yaml.Node
		if err := yaml.Unmarshal(raw, &tree); err != nil {
			t.Fatal(err)
		}
		count := 0
		for i := 0; i < len(tree.Content[0].Content); i += 2 {
			if tree.Content[0].Content[i].Value == "articles" {
				count++
			}
		}
		if count != 1 {
			t.Fatalf("articles key count = %d: %s", count, raw)
		}
		var value map[string]any
		if err := yaml.Unmarshal(raw, &value); err != nil {
			t.Fatal(err)
		}
		articles, ok := value["articles"].(map[string]any)
		if !ok {
			t.Fatalf("articles = %#v", value["articles"])
		}
		if _, ok := articles["second"]; !ok {
			t.Fatalf("last article lost: %s", raw)
		}
	})
	for _, test := range []struct{ name, body, title string }{
		{"load_succeeds_with_duplicate_keys", frozenDuplicateArticles, "Second"},
		{"load_succeeds_with_nested_duplicate_keys", "schema: '1'\narticles:\n  first:\n    title: Old\n    title: New\n    article_path: docs/first.md\n", "New"},
	} {
		t.Run(test.name, func(t *testing.T) {
			path := filepath.Join(t.TempDir(), "mind-index.yaml")
			if err := os.WriteFile(path, []byte(test.body), 0600); err != nil {
				t.Fatal(err)
			}
			document, err := ReadIndex(path, true)
			if err != nil || document.Value.Articles == nil || len(*document.Value.Articles) != 1 || (*document.Value.Articles)[0].Title != test.title {
				t.Fatalf("loaded = %+v, %v", document, err)
			}
			raw, err := os.ReadFile(path)
			if err != nil || string(raw) != test.body {
				t.Fatalf("read rewrote bytes = %q, %v", raw, err)
			}
		})
	}
	t.Run("merge_duplicate_top_level_keys_combines_article_mappings", func(t *testing.T) {
		raw, err := MergeDuplicateIndex([]byte(frozenDuplicateArticles))
		if err != nil {
			t.Fatal(err)
		}
		var index model.Index
		if err := yaml.Unmarshal(raw, &index); err != nil {
			t.Fatal(err)
		}
		if index.Articles == nil || len(*index.Articles) != 2 {
			t.Fatalf("merged = %+v", index.Articles)
		}
		names := map[string]bool{}
		for _, article := range *index.Articles {
			names[article.Title] = true
		}
		if !names["First"] || !names["Second"] {
			t.Fatalf("merged titles = %v", names)
		}
		duplicates, err := IndexDuplicates(raw)
		if err != nil || len(duplicates) != 0 {
			t.Fatalf("merged duplicates = %+v, %v", duplicates, err)
		}
	})
}

func TestIndexDuplicateLocationsComeFromYAMLNodes(t *testing.T) {
	raw := []byte("schema: '1'\narticles:\n  first: {title: First, article_path: docs/first.md}\n# duplicate below\narticles:\n  second: {title: Second, article_path: docs/second.md}\n")
	duplicates, err := IndexDuplicates(raw)
	if err != nil || len(duplicates) != 1 || duplicates[0].Key != "articles" || duplicates[0].Line != 5 {
		t.Fatalf("duplicate location = %+v, %v", duplicates, err)
	}
	for _, valid := range []string{"schema: '1'\narticles: {}\n", "schema: '1'\n"} {
		got, err := IndexDuplicates([]byte(valid))
		if err != nil || len(got) != 0 {
			t.Fatalf("spurious duplicate = %+v, %v", got, err)
		}
	}
}
