package build

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"

	output "github.com/alswl/mind-forge/pkg/common/outcome"
)

func TestBuildFrozenAssertions(t *testing.T) {
	t.Run("resolve_indexed_article_path_by_exact_key", func(t *testing.T) {
		root := buildTestRoot(t)
		putBuildFile(t, root, "mind.yaml", "schema: '1'\n")
		putBuildFile(t, root, "docs/2026-05-monthly/01-team-okr.md", "# okr\n")
		putBuildFile(t, root, "mind-index.yaml", "schema: '1'\narticles:\n- title: 2026 05 monthly\n  article_path: docs/2026-05-monthly\n")
		p, err := Prepare(root, root, "2026-05-monthly", Options{DryRun: true})
		if err != nil {
			t.Fatal(err)
		}
		info, err := os.Stat(p.Plan.ArticlePath)
		if err != nil || !info.IsDir() {
			t.Fatalf("resolved path: %s %v", p.Plan.ArticlePath, err)
		}
	})
	t.Run("resolve_indexed_article_path_title_not_used_as_path_derivation", func(t *testing.T) {
		root := buildTestRoot(t)
		putBuildFile(t, root, "mind.yaml", "schema: '1'\n")
		putBuildFile(t, root, "docs/team-updates/01-note.md", "# note\n")
		putBuildFile(t, root, "mind-index.yaml", "schema: '1'\narticles:\n- title: Team Updates\n  article_path: docs/team-updates\n")
		p, err := Prepare(root, root, "team-updates", Options{DryRun: true})
		if err != nil {
			t.Fatal(err)
		}
		if _, err := os.Stat(p.Plan.ArticlePath); err != nil {
			t.Fatal(err)
		}
		_, err = Prepare(root, root, "Team Updates", Options{DryRun: true})
		var failure *output.Error
		if !errors.As(err, &failure) || failure.Kind != "not_found" {
			t.Fatalf("display-title lookup: %v", err)
		}
	})
	t.Run("rewrite_target_skips_absolute_and_urls", func(t *testing.T) {
		warnings := []string{}
		for _, target := range []string{"/abs/x.png", "http://x/y.png", "https://x/y.png", "mailto:a@b.com", "data:image/png,AA", "#anchor", ""} {
			if _, ok := rewriteTarget(target, "docs/art", "outputs", &warnings); ok {
				t.Errorf("rewrote %q", target)
			}
		}
		if len(warnings) != 0 {
			t.Fatal(warnings)
		}
	})
	t.Run("rewrite_target_rebases_and_normalizes", func(t *testing.T) {
		warnings := []string{}
		for target, want := range map[string]string{"assets/p.png": "../docs/art/assets/p.png", "../shared/p.png": "../docs/shared/p.png"} {
			got, ok := rewriteTarget(target, "docs/art", filepath.Clean("outputs"), &warnings)
			if !ok || got != want || strings.Contains(got, "/../") {
				t.Fatalf("%q => %q %t", target, got, ok)
			}
		}
		if len(warnings) != 0 {
			t.Fatal(warnings)
		}
	})
	t.Run("rewrite_target_warns_on_mixed_base_instead_of_malformed_path", func(t *testing.T) {
		warnings := []string{}
		_, ok := rewriteTarget("assets/p.png", "/abs/docs/art", "outputs", &warnings)
		if ok || len(warnings) != 1 || !strings.Contains(warnings[0], "assets/p.png") {
			t.Fatalf("%t %v", ok, warnings)
		}
	})
	t.Run("rewrite_relative_paths_end_to_end", func(t *testing.T) {
		input := "![工作流全景](assets/p.png)\n\n```\n![untouched](x.png)\n```\n[img]: assets/q.png\n"
		want := "![工作流全景](../docs/art/assets/p.png)\n\n```\n![untouched](x.png)\n```\n[img]: ../docs/art/assets/q.png\n"
		warnings := []string{}
		got := rewriteRelativePaths(input, "docs/art", "outputs", &warnings)
		if got != want || len(warnings) != 0 {
			t.Fatalf("%q %v", got, warnings)
		}
	})
	t.Run("rewrite_relative_paths_keeps_unresolvable_reference_and_warns", func(t *testing.T) {
		input := "![hero](assets/p.png)\n"
		warnings := []string{}
		got := rewriteRelativePaths(input, "/abs/docs/art", "outputs", &warnings)
		if got != input || strings.Contains(got, "////") || len(warnings) != 1 {
			t.Fatalf("%q %v", got, warnings)
		}
	})
}
