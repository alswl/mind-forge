package source

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/alswl/mind-forge/internal/output"
)

func TestLegacySourceInputAnchors(t *testing.T) {
	for _, name := range []string{"resolves_each_anchor_and_reports_precedence", "cwd_is_the_final_fallback", "not_found_is_a_usage_error_naming_anchors", "missing_absolute_is_usage_not_io"} {
		t.Run(name, func(t *testing.T) {
			root := t.TempDir()
			project := filepath.Join(root, "projects", "demo")
			sources := filepath.Join(project, "sources")
			cwd := filepath.Join(root, "cwd")
			if err := os.MkdirAll(cwd, 0o755); err != nil {
				t.Fatal(err)
			}
			touch := func(path string) {
				t.Helper()
				if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
					t.Fatal(err)
				}
				if err := os.WriteFile(path, []byte("x"), 0o644); err != nil {
					t.Fatal(err)
				}
			}
			switch name {
			case "resolves_each_anchor_and_reports_precedence":
				path := filepath.Join(sources, "self-eval", "x.md")
				touch(path)
				for _, input := range []string{"self-eval/x.md", "sources/self-eval/x.md", path} {
					got, err := ResolveInput(input, project, sources, cwd, root)
					if err != nil || got != path {
						t.Fatalf("source input %q = %q, %v; want %q", input, got, err, path)
					}
				}
			case "cwd_is_the_final_fallback":
				path := filepath.Join(cwd, "only-here.md")
				touch(path)
				got, err := ResolveInput("only-here.md", project, sources, cwd, root)
				if err != nil || got != path {
					t.Fatalf("cwd fallback = %q, %v", got, err)
				}
			default:
				input := "nope/missing.md"
				if name == "missing_absolute_is_usage_not_io" {
					input = filepath.Join(root, "does", "not", "exist.md")
				}
				_, err := ResolveInput(input, project, sources, root, root)
				var typed *output.Error
				if !errors.As(err, &typed) || typed.Kind != "usage" {
					t.Fatalf("missing input error must be usage: %v", err)
				}
				if name == "not_found_is_a_usage_error_naming_anchors" && (!strings.Contains(err.Error(), "file not found") || !strings.Contains(err.Error(), "tried")) {
					t.Fatalf("missing anchor diagnostic: %v", err)
				}
			}
		})
	}
}
