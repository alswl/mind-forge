package content

import (
	"errors"
	"strings"
	"testing"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
)

func TestLegacyArticleOutputStem(t *testing.T) {
	t.Run("article_output_stem_strips_trailing_slash", func(t *testing.T) {
		for _, path := range []string{"docs/2026-09-monthly/", "docs/2026-09-monthly"} {
			if got := ArticleOutputStem(path); got != "2026-09-monthly" {
				t.Fatalf("stem(%s) = %s", path, got)
			}
		}
	})
	t.Run("article_output_stem_trailing_slash_matches_slash_free_form", func(t *testing.T) {
		if a, b := ArticleOutputStem("docs/dup/"), ArticleOutputStem("docs/dup"); a != b {
			t.Fatalf("stems differ: %q, %q", a, b)
		}
	})
}

func TestLegacyIndexedArticleResolution(t *testing.T) {
	for _, test := range []struct {
		name, path, selector string
		generated            bool
	}{
		{"resolve_by_exact_key_declared", "docs/2026-05-monthly", "docs/2026-05-monthly", false},
		{"resolve_by_docs_dir_relative_name", "docs/2026-05-monthly", "2026-05-monthly", false},
		{"resolve_by_template_alias", "outputs/2026-05/2026-05-15.md", "daily_report/2026-05-15", true},
		{"resolve_by_exact_key_generated", "outputs/2026-05/2026-05-15.md", "outputs/2026-05/2026-05-15", true},
		{"resolve_by_exact_article_path", "docs/my-article.md", "docs/my-article.md", false},
	} {
		t.Run(test.name, func(t *testing.T) {
			article := model.Article{Title: "test", Path: test.path, Project: "test", Type: "blog", Status: "draft", CreatedAt: "2026-05-15T00:00:00Z", UpdatedAt: "2026-05-15T00:00:00Z"}
			if test.generated {
				article.TemplateOrigin = &model.TemplateOrigin{TemplateName: "daily_report", SlotValue: "2026-05-15"}
			}
			got, err := ResolveIndexedEntries([]model.Article{article}, test.selector)
			if err != nil || got.Path != test.path {
				t.Fatalf("resolved = %+v, %v, want %q", got, err, test.path)
			}
		})
	}
	for _, test := range []struct{ name, path, title, selector string }{
		{"resolve_not_found", "docs/only-article.md", "test", "missing-article"},
		{"resolve_title_not_used", "docs/2026-05-monthly", "2026 05 monthly", "2026 05 monthly"},
	} {
		t.Run(test.name, func(t *testing.T) {
			_, err := ResolveIndexedEntries([]model.Article{{Path: test.path, Title: test.title}}, test.selector)
			var failure *output.Error
			if !errors.As(err, &failure) || failure.Kind != "not_found" {
				t.Fatalf("rejected lookup = %v", err)
			}
		})
	}
}

func TestLegacyArticleIndexComponents(t *testing.T) {
	t.Run("compute_article_diff_trailing_slash_article_path_is_neither_added_nor_removed", func(t *testing.T) {
		path := "docs/2026-09-monthly"
		articles := []model.Article{{Title: "Monthly Report", Project: "demo", Type: "blank", Path: path + "/", Status: "draft", CreatedAt: "2020-01-01T00:00:00Z", UpdatedAt: "2020-01-01T00:00:00Z"}}
		scanned := []model.ScannedArticle{{Title: "2026 09 monthly", Filename: "2026-09-monthly", ArticleDir: &path, ArticlePath: &path}}
		diff := ArticleDiff(model.Index{Articles: &articles}, scanned, "docs")
		if len(diff.Added) != 0 || len(diff.Removed) != 0 {
			t.Fatalf("diff = %+v", diff)
		}
	})
	t.Run("scan_docs_title_is_derived_from_slug_not_from_h1", func(t *testing.T) {
		project := t.TempDir()
		putContent(t, project, "docs/2026-08-skm.md", "# SKM：个人中心的 Agent Skill 管理器\n\nBody.\n")
		config := model.DefaultMindConfig()
		articles, err := ScanArticleDocs(project, &config)
		if err != nil || len(articles) != 1 {
			t.Fatalf("articles = %+v, %v", articles, err)
		}
		if articles[0].Title != "2026 08 skm" || strings.Contains(articles[0].Title, "SKM") {
			t.Fatalf("title = %q", articles[0].Title)
		}
	})
}
