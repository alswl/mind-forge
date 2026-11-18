package content

import (
	"errors"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
	"time"

	output "github.com/alswl/mind-forge/pkg/common/outcome"
)

const frozenBlogTemplate = "# {title}\n\n> Created: {created_at}\n\n## Summary\n\n## Content\n"

func TestLegacyArticleBuiltinTemplates(t *testing.T) {
	for _, test := range []struct {
		name, template string
		contains       []string
	}{
		{"builtin_blank", "blank", []string{"{title}"}},
		{"builtin_arch", "arch", []string{"## Context", "## Decision", "## Consequence", "## Alternatives Considered"}},
		{"builtin_prd", "prd", []string{"## Background", "## Goals"}},
		{"builtin_blog_matches_legacy_article_template", "blog", nil},
		{"builtin_blog_body_matches_legacy_byte_for_byte", "blog", nil},
	} {
		t.Run(test.name, func(t *testing.T) {
			body, kind, ok := BuiltinArticleTemplate(test.template)
			if !ok || kind != test.template {
				t.Fatalf("template %s = %q, %v", test.template, kind, ok)
			}
			for _, text := range test.contains {
				if !strings.Contains(body, text) {
					t.Fatalf("missing %q in %q", text, body)
				}
			}
			if test.template == "blog" && body != frozenBlogTemplate {
				t.Fatalf("blog bytes = %q", body)
			}
		})
	}
	t.Run("builtin_unknown_returns_none", func(t *testing.T) {
		for _, name := range []string{"nope", "ARCH", ""} {
			if _, _, ok := BuiltinArticleTemplate(name); ok {
				t.Fatalf("unexpected template %q", name)
			}
		}
	})
	t.Run("builtin_h2_slugs_are_distinct", func(t *testing.T) {
		for _, name := range []string{"arch", "prd", "blog"} {
			body, _, ok := BuiltinArticleTemplate(name)
			if !ok {
				t.Fatal(name)
			}
			seen := map[string]bool{}
			for _, line := range strings.Split(body, "\n") {
				if heading, ok := strings.CutPrefix(line, "## "); ok {
					slug := ArticleFilename(strings.TrimSpace(heading))
					if seen[slug] {
						t.Fatalf("duplicate %s in %s", slug, name)
					}
					seen[slug] = true
				}
			}
		}
	})
}

func TestLegacyArticleTemplateSplit(t *testing.T) {
	t.Run("split_zero_h2_produces_head_only", func(t *testing.T) {
		got, err := SplitArticleTemplate("# Title\n\nBody text\n")
		want := []ArticleBlock{{"01-opening.md", "# Title\n\nBody text"}}
		if err != nil || !reflect.DeepEqual(got, want) {
			t.Fatalf("blocks = %#v, %v", got, err)
		}
	})
	t.Run("split_four_h2_arch_style", func(t *testing.T) {
		input := "# Title\n\n> Created: now\n\n## Context\n\nctx body\n\n## Decision\n\ndec\n\n## Consequence\n\ncons\n\n## Alternatives Considered\n\nalt\n"
		got, err := SplitArticleTemplate(input)
		if err != nil || len(got) != 5 {
			t.Fatalf("blocks = %#v, %v", got, err)
		}
		for i, name := range []string{"01-opening.md", "02-context.md", "03-decision.md", "04-consequence.md", "05-alternatives-considered.md"} {
			if got[i].Filename != name {
				t.Fatalf("block %d = %#v", i, got[i])
			}
		}
		if !strings.HasPrefix(got[0].Body, "# Title") || !strings.HasPrefix(got[1].Body, "## Context") {
			t.Fatalf("blocks = %#v", got)
		}
	})
	t.Run("split_leading_h2_empty_head", func(t *testing.T) {
		got, err := SplitArticleTemplate("## Intro\n\nintro body\n")
		want := []ArticleBlock{{"01-opening.md", ""}, {"02-intro.md", "## Intro\n\nintro body"}}
		if err != nil || !reflect.DeepEqual(got, want) {
			t.Fatalf("blocks = %#v, %v", got, err)
		}
	})
	t.Run("split_crlf_normalized", func(t *testing.T) {
		got, err := SplitArticleTemplate("# Title\r\n\r\n## Summary\r\n\r\nbody\r\n")
		if err != nil || len(got) != 2 {
			t.Fatalf("blocks = %#v, %v", got, err)
		}
		for _, block := range got {
			if strings.Contains(block.Body, "\r") {
				t.Fatalf("CR retained: %#v", block)
			}
		}
	})
	t.Run("split_duplicate_slug_rejected", func(t *testing.T) {
		_, err := SplitArticleTemplate("# T\n\n## Notes\n\nbody1\n\n## NOTES\n\nbody2\n")
		var failure *output.Error
		if !errors.As(err, &failure) || failure.Kind != "duplicate_block_slug" {
			t.Fatalf("duplicate = %v", err)
		}
	})
	t.Run("split_single_h2", func(t *testing.T) {
		got, err := SplitArticleTemplate("# Title\n\nintro\n\n## Summary\n\nsummary body\n")
		if err != nil || len(got) != 2 {
			t.Fatalf("blocks = %#v, %v", got, err)
		}
		if got[0].Filename != "01-opening.md" || got[0].Body != "# Title\n\nintro\n" || got[1].Filename != "02-summary.md" || !strings.HasPrefix(got[1].Body, "## Summary") {
			t.Fatalf("blocks = %#v", got)
		}
	})
	t.Run("split_ignores_h2_inside_fenced_code_block", func(t *testing.T) {
		got, err := SplitArticleTemplate("# T\n\n## Real One\n\n```md\n## Not A Heading\nmore fenced\n```\n\ntext\n\n## Real Two\n\nbody\n")
		if err != nil || len(got) != 3 {
			t.Fatalf("blocks = %#v, %v", got, err)
		}
		if got[1].Filename != "02-real-one.md" || !strings.Contains(got[1].Body, "## Not A Heading") || got[2].Filename != "03-real-two.md" {
			t.Fatalf("blocks = %#v", got)
		}
	})
	t.Run("split_cjk_headings_produce_distinct_slugs", func(t *testing.T) {
		got, err := SplitArticleTemplate("# 标题\n\n## 本周进展\n\na\n\n## 里程碑规划与进展\n\nb\n")
		if err != nil || len(got) != 3 {
			t.Fatalf("blocks = %#v, %v", got, err)
		}
		if got[1].Filename != "02-本周进展.md" || got[2].Filename != "03-里程碑规划与进展.md" {
			t.Fatalf("blocks = %#v", got)
		}
	})
}

func TestLegacyArticleSelectors(t *testing.T) {
	for _, test := range []struct{ name, title, path, selector string }{
		{"resolves_full_article_path", "Monthly", "docs/2026-09-monthly.md", "docs/2026-09-monthly.md"},
		{"resolves_article_path_without_md_suffix", "Monthly", "docs/2026-09-monthly.md", "docs/2026-09-monthly"},
		{"resolves_bare_slug", "Monthly", "docs/2026-09-monthly.md", "2026-09-monthly"},
		{"resolves_bare_slug_for_directory_article", "Monthly", "docs/2026-09-monthly", "2026-09-monthly"},
		{"resolves_exact_title", "Monthly Report", "docs/2026-09-monthly.md", "Monthly Report"},
		{"not_found_reports_hint", "Monthly", "docs/2026-09-monthly.md", "nonexistent"},
		{"contains_substring_does_not_match", "Monthly", "docs/2026-09-monthly.md", "2026-09"},
	} {
		t.Run(test.name, func(t *testing.T) {
			project := t.TempDir()
			putContent(t, project, "mind.yaml", "schema: '1'\n")
			putContent(t, project, "mind-index.yaml", "schema: '1'\narticles:\n  docs/2026-09-monthly:\n    title: "+test.title+"\n    article_path: "+test.path+"\n")
			got, err := ResolveArticleSelector(project, test.selector)
			if test.name == "not_found_reports_hint" || test.name == "contains_substring_does_not_match" {
				if err == nil || !strings.Contains(err.Error(), "not found") {
					t.Fatalf("selector = %q, %v", got, err)
				}
				return
			}
			if err != nil || got != test.path {
				t.Fatalf("selector = %q, %v, want %q", got, err, test.path)
			}
		})
	}
	t.Run("ambiguous_when_directory_and_single_file_share_a_slug", func(t *testing.T) {
		project := t.TempDir()
		putContent(t, project, "mind.yaml", "schema: '1'\n")
		putContent(t, project, "mind-index.yaml", "schema: '1'\narticles:\n  docs/dup:\n    title: Dup Dir\n    article_path: docs/dup\n  outputs/dup:\n    title: Dup Generated\n    article_path: outputs/dup.md\n")
		_, err := ResolveArticleSelector(project, "dup")
		if err == nil || !strings.Contains(err.Error(), "ambiguous") || !strings.Contains(err.Error(), "docs/dup") || !strings.Contains(err.Error(), "outputs/dup.md") {
			t.Fatalf("ambiguity = %v", err)
		}
	})
}

func TestLegacyArticleFileMtime(t *testing.T) {
	t.Run("mtime_regular_file", func(t *testing.T) {
		project := t.TempDir()
		putContent(t, project, "doc.md", "content")
		if got := ArticleFileMtime(project, "doc.md"); got == 0 {
			t.Fatal("regular file has zero mtime")
		}
	})
	t.Run("mtime_missing_path_returns_zero", func(t *testing.T) {
		if got := ArticleFileMtime(t.TempDir(), "nonexistent.md"); got != 0 {
			t.Fatalf("missing mtime = %d", got)
		}
	})
	t.Run("mtime_directory_returns_newest_file", func(t *testing.T) {
		project := t.TempDir()
		putContent(t, project, "docs/a.md", "old")
		putContent(t, project, "docs/b.md", "newer")
		for name, seconds := range map[string]int64{"a.md": 1700000000, "b.md": 1700000100} {
			date := time.Unix(seconds, 0)
			if err := os.Chtimes(filepath.Join(project, "docs", name), date, date); err != nil {
				t.Fatal(err)
			}
		}
		var expected uint64
		for _, name := range []string{"a.md", "b.md"} {
			info, err := os.Stat(filepath.Join(project, "docs", name))
			if err != nil {
				t.Fatal(err)
			}
			expected = max(expected, uint64(info.ModTime().Unix()))
		}
		if got := ArticleFileMtime(project, "docs"); got == 0 || got != expected {
			t.Fatalf("directory mtime = %d, want %d", got, expected)
		}
	})
	t.Run("mtime_empty_directory_returns_zero", func(t *testing.T) {
		project := t.TempDir()
		if err := os.Mkdir(filepath.Join(project, "empty_article"), 0700); err != nil {
			t.Fatal(err)
		}
		if got := ArticleFileMtime(project, "empty_article"); got != 0 {
			t.Fatalf("empty directory mtime = %d", got)
		}
	})
}
