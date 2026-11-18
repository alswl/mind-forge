package output

import (
	"reflect"
	"strings"
	"testing"
)

func TestLegacyVerbText(t *testing.T) {
	old, path := "demo", "projects/demo"
	for _, test := range []struct {
		name   string
		result VerbResult
		want   string
	}{
		{"create_text", VerbResult{Verb: Create, Kind: "project", Identity: "demo", Path: &path, Details: map[string]any{"scaffolded": []any{"docs"}}}, "✓ created project: demo"},
		{"create_dry_run_text", VerbResult{Verb: Create, Kind: "project", Identity: "demo", Path: &path, DryRun: true, Details: map[string]any{}}, "[dry-run] would create project: demo"},
		{"rename_text", VerbResult{Verb: Rename, Kind: "project", Identity: "demo-renamed", OldIdentity: &old, Details: map[string]any{}}, "✓ renamed project: demo → demo-renamed"},
		{"remove_text", VerbResult{Verb: Remove, Kind: "article", Identity: "docs/draft", Details: map[string]any{}}, "✓ removed article: docs/draft"},
		{"update_text", VerbResult{Verb: Update, Kind: "source", Identity: "report", Details: map[string]any{"changes": map[string]any{"title": map[string]any{"from": "Old", "to": "New"}}}}, "✓ updated source: report (1 field)"},
		{"index_text", VerbResult{Verb: Index, Kind: "article", Details: map[string]any{"added": []any{map[string]any{"identity": "a"}}, "removed": []any{map[string]any{"identity": "b"}}, "kept_count": 5, "scanned_count": 7}}, "indexed article: +1 =5 -1"},
	} {
		t.Run(test.name, func(t *testing.T) {
			if got := RenderVerb(test.result, DisplayOptions{}); got != test.want {
				t.Fatalf("verb text = %q, want %q", got, test.want)
			}
		})
	}
	for _, name := range []string{"lint_text", "lint_path_renders_as_link", "lint_path_no_hyperlinks_returns_plain"} {
		t.Run(name, func(t *testing.T) {
			line := 0
			opts := DisplayOptions{}
			if name == "lint_path_renders_as_link" {
				line = 42
				opts = DisplayOptions{RepoRoot: "/repo", Hyperlinks: true}
			}
			r := VerbResult{Verb: Lint, Kind: "article", Details: map[string]any{"issues": []any{map[string]any{"severity": "error", "kind": "missing_directory", "message": "docs/foo not on disk", "path": "docs/foo", "line": line}}, "summary": map[string]any{"errors": 1, "warnings": 0, "info": 0, "fixed": 0}}}
			got := RenderVerb(r, opts)
			switch name {
			case "lint_text":
				if !strings.Contains(got, "[error] missing_directory: docs/foo not on disk (docs/foo)") || !strings.Contains(got, "1 errors, 0 warnings, 0 info") {
					t.Fatalf("lint text changed: %q", got)
				}
			case "lint_path_renders_as_link":
				if !strings.Contains(got, "\x1b]8;;file:///repo/docs/foo\x1b\\") || !strings.Contains(got, "\x1b]8;;\x1b\\:42") {
					t.Fatalf("lint link/line boundary changed: %q", got)
				}
			default:
				if strings.ContainsRune(got, '\x1b') || !strings.Contains(got, "docs/foo") {
					t.Fatalf("plain lint path changed: %q", got)
				}
			}
		})
	}
}

func TestLegacyVerbEnvelopes(t *testing.T) {
	old, path := "rag", "projects/demo"
	for _, test := range []struct {
		name   string
		result VerbResult
		want   map[string]any
	}{
		{"json_envelope_create", VerbResult{Verb: Create, Kind: "project", Identity: "demo", Path: &path, Details: map[string]any{"scaffolded": []any{"docs"}}}, map[string]any{"kind": "project", "identity": "demo", "dry_run": false, "path": "projects/demo", "details": map[string]any{"scaffolded": []any{"docs"}}}},
		{"json_envelope_rename", VerbResult{Verb: Rename, Kind: "term", Identity: "RAG", OldIdentity: &old, Details: map[string]any{"keep_alias": true}}, map[string]any{"kind": "term", "new_identity": "RAG", "old_identity": "rag", "details": map[string]any{"keep_alias": true}}},
		{"json_envelope_remove", VerbResult{Verb: Remove, Kind: "article", Identity: "docs/draft", Details: map[string]any{}}, map[string]any{"removed": true}},
	} {
		t.Run(test.name, func(t *testing.T) {
			got := VerbEnvelope(test.result)
			for key, want := range test.want {
				value, ok := got[key]
				if !ok || !reflect.DeepEqual(value, want) {
					t.Fatalf("envelope[%q] = %#v, want %#v", key, value, want)
				}
			}
		})
	}
}
