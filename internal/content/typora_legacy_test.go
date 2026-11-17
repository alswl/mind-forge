package content

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestLegacyTyporaInjection(t *testing.T) {
	for _, test := range []struct {
		name, body string
		contains   []string
		missing    string
		delimiters int
	}{
		{"inject_no_front_matter_prepend", "# Title\n\nBody\n", []string{"typora-copy-images-to: ../assets\n", "# Title"}, "", 0},
		{"inject_existing_front_matter_merge", "---\ntitle: Test\n---\n# Body\n", []string{"typora-copy-images-to: ../assets\n", "title: Test\n", "# Body"}, "", 2},
		{"inject_existing_typora_value_preserved", "---\ntitle: Test\ntypora-copy-images-to: ../../media\n---\n# Body\n", []string{"typora-copy-images-to: ../../media\n"}, "typora-copy-images-to: ../assets\n", 0},
		{"inject_existing_front_matter_with_crlf_merge", "---\r\ntitle: Test\r\n---\r\n# Body\r\n", []string{"title: Test\r\n", "typora-copy-images-to: ../assets\r\n", "---\r\n# Body\r\n"}, "", 0},
		{"inject_comment_mentioning_typora_key_does_not_count_as_existing_value", "---\n# typora-copy-images-to: old\ntitle: Test\n---\n# Body\n", []string{"# typora-copy-images-to: old\n", "typora-copy-images-to: ../assets\n"}, "", 0},
	} {
		t.Run(test.name, func(t *testing.T) {
			got := InjectTyporaFrontMatter(test.body, "../assets")
			for _, fragment := range test.contains {
				if !strings.Contains(got, fragment) {
					t.Fatalf("missing %q in %q", fragment, got)
				}
			}
			if test.missing != "" && strings.Contains(got, test.missing) {
				t.Fatalf("unexpected %q in %q", test.missing, got)
			}
			if test.delimiters > 0 && strings.Count(got, "---") != test.delimiters {
				t.Fatalf("front matter delimiters: %q", got)
			}
			if test.name == "inject_no_front_matter_prepend" && !strings.HasPrefix(got, "---\n") {
				t.Fatalf("opening: %q", got)
			}
			if test.name == "inject_existing_front_matter_with_crlf_merge" && !strings.HasPrefix(got, "---\r\n") {
				t.Fatalf("CRLF opening: %q", got)
			}
		})
	}
}

func TestLegacyTyporaAssetsPath(t *testing.T) {
	t.Run("compute_path_absolute", func(t *testing.T) {
		got, err := typoraAssetsPath("/Users/me/proj", "/static/images", "/Users/me/proj/docs/post")
		if err != nil || got != "/static/images" {
			t.Fatalf("absolute = %q, %v", got, err)
		}
	})
	for _, test := range []struct{ name, dir, want string }{
		{"compute_path_root_relative", "docs", "../assets"},
		{"compute_path_directory_article_relative", "docs/my-post", "../../assets"},
		{"compute_path_single_file_relative", "docs", "../assets"},
	} {
		t.Run(test.name, func(t *testing.T) {
			project := t.TempDir()
			for _, dir := range []string{"assets", test.dir} {
				if err := os.MkdirAll(filepath.Join(project, dir), 0700); err != nil {
					t.Fatal(err)
				}
			}
			got, err := typoraAssetsPath(project, "assets", filepath.Join(project, test.dir))
			if err != nil || got != test.want {
				t.Fatalf("relative = %q, %v, want %q", got, err, test.want)
			}
		})
	}
}

func TestLegacyTyporaEnabledPolicy(t *testing.T) {
	for _, test := range []struct {
		name, plugins string
		want          bool
	}{
		{"effective_enabled_default", "", true},
		{"effective_enabled_missing_plugin", "plugins: {}\n", true},
		{"effective_enabled_explicit_true", "plugins:\n  typora-front-matter: {enabled: true}\n", true},
		{"effective_enabled_explicit_false", "plugins:\n  typora-front-matter: {enabled: false}\n", false},
	} {
		t.Run(test.name, func(t *testing.T) {
			project := t.TempDir()
			putContent(t, project, "mind.yaml", "schema: '1'\n"+test.plugins)
			got, err := CreateArticle(project, "Test", ArticleCreateOptions{DryRun: true})
			if err != nil {
				t.Fatal(err)
			}
			if got.TyporaFrontMatterInjected != test.want || (got.TyporaCopyImagesTo != nil) != test.want {
				t.Fatalf("Typora result = %+v, want enabled %v", got, test.want)
			}
			if _, err := os.Stat(filepath.Join(project, "docs")); !os.IsNotExist(err) {
				t.Fatalf("preview materialized docs: %v", err)
			}
		})
	}
}
