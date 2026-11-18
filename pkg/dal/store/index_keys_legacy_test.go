package store

import (
	"errors"
	"os"
	"path/filepath"
	"reflect"
	"testing"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
	"go.yaml.in/yaml/v3"
)

func TestLegacyArticleIndexKeys(t *testing.T) {
	for _, test := range []struct {
		name, path, key string
		generated       bool
	}{
		{"article_key_uses_article_path_not_template_origin", "outputs/2026-05/2026-05-15.md", "outputs/2026-05/2026-05-15", true},
		{"article_key_returns_full_path_for_docs_article", "docs/weekly-summary.md", "docs/weekly-summary", false},
		{"article_key_strips_md_keeps_docs_prefix", "docs/my-article.md", "docs/my-article", false},
		{"article_key_generated_has_article_path_key", "outputs/2026-05/2026-05-15.md", "outputs/2026-05/2026-05-15", true},
		{"article_key_handles_directory_article_path", "docs/2026-05-monthly", "docs/2026-05-monthly", false},
		{"article_key_handles_non_docs_directory_article_path", "specs/quarterly", "specs/quarterly", false},
		{"resolve_by_exact_key_declared", "docs/2026-05-monthly", "docs/2026-05-monthly", false},
		{"resolve_by_docs_dir_relative_name", "docs/2026-05-monthly", "docs/2026-05-monthly", false},
		{"resolve_by_template_alias", "outputs/2026-05/2026-05-15.md", "outputs/2026-05/2026-05-15", true},
		{"resolve_by_exact_key_generated", "outputs/2026-05/2026-05-15.md", "outputs/2026-05/2026-05-15", true},
		{"resolve_by_exact_article_path", "docs/my-article.md", "docs/my-article", false},
	} {
		t.Run(test.name, func(t *testing.T) {
			article := model.Article{Title: "test", Path: test.path, Project: "test", Type: "blog", Status: "draft", CreatedAt: "2026-05-15T00:00:00Z", UpdatedAt: "2026-05-15T00:00:00Z"}
			if test.generated {
				article.TemplateOrigin = &model.TemplateOrigin{TemplateName: "daily_report", SlotValue: "2026-05-15"}
			}
			articles := []model.Article{article}
			raw, err := renderIndexSection("articles", &articles)
			if err != nil {
				t.Fatal(err)
			}
			var document map[string]map[string]any
			if err := yaml.Unmarshal(raw, &document); err != nil {
				t.Fatal(err)
			}
			if len(document["articles"]) != 1 {
				t.Fatalf("articles = %#v", document)
			}
			if _, exists := document["articles"][test.key]; !exists {
				t.Fatalf("key %q absent: %s", test.key, raw)
			}
		})
	}
}

func TestLegacyIndexLoadErrors(t *testing.T) {
	t.Run("load_returns_default_when_file_missing", func(t *testing.T) {
		got, err := ReadIndex(filepath.Join(t.TempDir(), "mind-index.yaml"), false)
		if err != nil || !reflect.DeepEqual(got.Value, model.DefaultIndex()) {
			t.Fatalf("missing index = %+v, %v", got, err)
		}
	})
	for _, test := range []struct{ name, body, kind string }{
		{"load_returns_incompatible_schema_on_unsupported_version", "schema_version: \"99\"\n", "incompatible_schema"},
		{"load_returns_parse_error_on_corrupt_yaml", "not: : valid: yaml", "parse_error"},
	} {
		t.Run(test.name, func(t *testing.T) {
			path := filepath.Join(t.TempDir(), "mind-index.yaml")
			if err := os.WriteFile(path, []byte(test.body), 0600); err != nil {
				t.Fatal(err)
			}
			_, err := ReadIndex(path, false)
			var failure *output.Error
			if !errors.As(err, &failure) || failure.Kind != test.kind {
				t.Fatalf("load = %v, want %s", err, test.kind)
			}
		})
	}
}
