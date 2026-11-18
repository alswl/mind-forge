package content

import (
	"os"
	"path/filepath"
	"reflect"
	"testing"
	"time"

	"github.com/alswl/mind-forge/pkg/common/model"
)

func legacyDeclaredConfig(typed map[string]*string, compat any) model.MindConfig {
	config := model.DefaultMindConfig()
	config.Build.Articles = model.JSONObject[model.ArticleBuildConfig]{}
	for id, dir := range typed {
		config.Build.Articles[id] = model.ArticleBuildConfig{ArticleDir: dir}
	}
	config.Articles = compat
	return config
}

func TestLegacyDeclaredArticles(t *testing.T) {
	for _, test := range []struct {
		name         string
		typed        map[string]*string
		compat       any
		dirs         []string
		files        map[string]string
		paths        map[string]string
		titles       []string
		originAbsent bool
	}{
		{name: "scan_declared_typed_only_returns_typed_id_with_article_dir_resolved_path", typed: map[string]*string{"reports": ptrString("reports")}, paths: map[string]string{"reports": "docs/reports.md"}, titles: []string{"reports"}, originAbsent: true},
		{name: "scan_declared_compat_only_returns_compat_id_with_docs_fallback", compat: map[string]any{"legacy-blog": map[string]any{"type": "blog"}}, paths: map[string]string{"legacy-blog": "docs/legacy-blog.md"}, titles: []string{"legacy blog"}},
		{name: "scan_declared_typed_wins_over_compat_on_id_collision", typed: map[string]*string{"shared": ptrString("typed-path")}, compat: map[string]any{"shared": map[string]any{"article_dir": "compat-path"}}, paths: map[string]string{"shared": "docs/shared.md"}},
		{name: "scan_declared_sorts_alphabetically_by_id", typed: map[string]*string{"zulu": ptrString("zulu/src"), "alpha": ptrString("alpha/src"), "beta": ptrString("beta/src")}, paths: map[string]string{"zulu": "docs/zulu.md", "alpha": "docs/alpha.md", "beta": "docs/beta.md"}, titles: []string{"alpha", "beta", "zulu"}},
		{name: "scan_declared_missing_source_still_returns_article", typed: map[string]*string{"ghost": ptrString("nonexistent-dir")}, paths: map[string]string{"ghost": "docs/ghost.md"}, titles: []string{"ghost"}},
		{name: "scan_declared_template_origin_always_none", typed: map[string]*string{"reports": ptrString("reports")}, paths: map[string]string{"reports": "docs/reports.md"}, originAbsent: true},
		{name: "scan_declared_prefers_docs_dir_over_md_file", typed: map[string]*string{"2026-05-monthly": nil}, dirs: []string{"docs/2026-05-monthly"}, paths: map[string]string{"2026-05-monthly": "docs/2026-05-monthly"}},
		{name: "scan_declared_uses_md_file_when_dir_absent", typed: map[string]*string{"my-article": nil}, files: map[string]string{"docs/my-article.md": "# content\n"}, paths: map[string]string{"my-article": "docs/my-article.md"}},
		{name: "scan_declared_directory_precedence_over_file", typed: map[string]*string{"my-article": nil}, dirs: []string{"docs/my-article"}, files: map[string]string{"docs/my-article.md": "# content\n"}, paths: map[string]string{"my-article": "docs/my-article"}},
		{name: "scan_declared_resolve_docs_fallback_for_config_no_article_dir", typed: map[string]*string{"team-updates": nil}, dirs: []string{"docs/team-updates"}, paths: map[string]string{"team-updates": "docs/team-updates"}},
		{name: "scan_declared_configured_article_dir_dir_article", typed: map[string]*string{"quarterly-review": ptrString("specs/quarterly")}, dirs: []string{"specs/quarterly"}, paths: map[string]string{"quarterly-review": "specs/quarterly"}},
		{name: "scan_declared_configured_article_dir_with_id_file", typed: map[string]*string{"my-article": ptrString("specs")}, files: map[string]string{"specs/my-article.md": "# spec\n"}, paths: map[string]string{"my-article": "specs/my-article.md"}},
	} {
		t.Run(test.name, func(t *testing.T) {
			project := t.TempDir()
			for _, dir := range test.dirs {
				if err := os.MkdirAll(filepath.Join(project, dir), 0700); err != nil {
					t.Fatal(err)
				}
			}
			for name, body := range test.files {
				putContent(t, project, name, body)
			}
			config := legacyDeclaredConfig(test.typed, test.compat)
			paths, err := declaredArticlePaths(project, &config, "docs")
			if err != nil || !reflect.DeepEqual(paths, test.paths) {
				t.Fatalf("declarations = %v, %v, want %v", paths, err, test.paths)
			}
			if test.titles != nil || test.originAbsent {
				index, _, err := DiscoverArticles(project, &config, time.Time{})
				if err != nil || index.Articles == nil || len(*index.Articles) != len(test.paths) {
					t.Fatalf("declared-only view = %+v, %v", index.Articles, err)
				}
				titles := []string{}
				for _, article := range *index.Articles {
					titles = append(titles, article.Title)
					if test.originAbsent && article.TemplateOrigin != nil {
						t.Fatalf("invented template origin: %+v", article)
					}
				}
				if test.titles != nil && !reflect.DeepEqual(titles, test.titles) {
					t.Fatalf("titles = %v, want %v", titles, test.titles)
				}
			}
		})
	}
}

func TestDeclaredTypedDirectoryBeatsExistingCompatDirectory(t *testing.T) {
	project := t.TempDir()
	for _, dir := range []string{"typed-path", "compat-path"} {
		if err := os.Mkdir(filepath.Join(project, dir), 0700); err != nil {
			t.Fatal(err)
		}
	}
	config := legacyDeclaredConfig(map[string]*string{"shared": ptrString("typed-path")}, map[string]any{"shared": map[string]any{"article_dir": "compat-path"}})
	paths, err := declaredArticlePaths(project, &config, "docs")
	if err != nil || !reflect.DeepEqual(paths, map[string]string{"shared": "typed-path"}) {
		t.Fatalf("typed precedence = %v, %v", paths, err)
	}
}

func TestDeclaredArticlesClearStaleTemplateOrigin(t *testing.T) {
	project := t.TempDir()
	config := legacyDeclaredConfig(map[string]*string{"reports": ptrString("reports")}, nil)
	putContent(t, project, "mind-index.yaml", "schema: '1'\narticles:\n  docs/reports:\n    title: Reports\n    article_path: docs/reports.md\n    template_origin: {template_name: daily, slot_value: '2026-05-15'}\n")
	index, _, err := DiscoverArticles(project, &config, time.Time{})
	if err != nil || index.Articles == nil || len(*index.Articles) != 1 {
		t.Fatalf("discovery = %+v, %v", index.Articles, err)
	}
	if got := (*index.Articles)[0]; got.Path != "docs/reports.md" || got.TemplateOrigin != nil {
		t.Fatalf("stale origin retained = %+v", got)
	}
}
