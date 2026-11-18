package store

import (
	"bytes"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
)

const frozenSubstringIndex = "schema: '1'\nterms:\n- term: TEST\n  corrections:\n    - original: aidc\n      correct: AIDC\n      match: substring\n"

func TestLegacyIndexCorrectionLoads(t *testing.T) {
	for _, test := range []struct {
		name   string
		strict bool
	}{
		{"strict_load_accepts_supported_substring_correction", true},
		{"lenient_load_allows_supported_substring_without_rewriting_file", false},
	} {
		t.Run(test.name, func(t *testing.T) {
			path := filepath.Join(t.TempDir(), "mind-index.yaml")
			if err := os.WriteFile(path, []byte(frozenSubstringIndex), 0600); err != nil {
				t.Fatal(err)
			}
			document, err := ReadIndex(path, test.strict)
			if err != nil {
				t.Fatal(err)
			}
			if document.Value.Terms == nil || len(*document.Value.Terms) != 1 || len((*document.Value.Terms)[0].Corrections) != 1 {
				t.Fatalf("terms = %+v", document.Value.Terms)
			}
			correction := (*document.Value.Terms)[0].Corrections[0]
			if correction.Match != "substring" || correction.Boundary != "standalone" {
				t.Fatalf("correction = %+v", correction)
			}
			raw, err := os.ReadFile(path)
			if err != nil || string(raw) != frozenSubstringIndex {
				t.Fatalf("load rewrote bytes: %q, %v", raw, err)
			}
		})
	}
	for _, test := range []struct{ name, original string }{
		{"boundary_standalone_with_original_ending_hyphen_rejected", "aidc-"},
		{"boundary_standalone_with_original_starting_underscore_rejected", "_aidc"},
	} {
		t.Run(test.name, func(t *testing.T) {
			path := filepath.Join(t.TempDir(), "mind-index.yaml")
			body := "schema: '1'\nterms:\n- term: TEST\n  corrections:\n    - original: " + test.original + "\n      correct: AIDC\n      boundary: standalone\n"
			if err := os.WriteFile(path, []byte(body), 0600); err != nil {
				t.Fatal(err)
			}
			_, err := ReadIndex(path, true)
			var failure *output.Error
			if !errors.As(err, &failure) || failure.ExitCode() != 2 || !strings.Contains(failure.Message, "identifier-character edges") || !strings.Contains(failure.Message, test.original) {
				t.Fatalf("invalid edge = %v", err)
			}
		})
	}
}

func TestLegacyUnknownSourceKindIndexRoundTrip(t *testing.T) {
	original, err := os.ReadFile("testdata/index-article-prompt-legacy.yaml")
	if err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(t.TempDir(), "mind-index.yaml")
	if err := os.WriteFile(path, original, 0600); err != nil {
		t.Fatal(err)
	}
	document, err := ReadIndex(path, true)
	if err != nil {
		t.Fatal(err)
	}
	if document.Value.Sources == nil || len(*document.Value.Sources) != 1 {
		t.Fatalf("sources = %+v", document.Value.Sources)
	}
	source := (*document.Value.Sources)[0]
	if source.SourceKind == nil || *source.SourceKind != "article_prompt" {
		t.Fatalf("source kind = %v", source.SourceKind)
	}
	addedPath := "sources/new.md"
	updated := document.Value
	sources := append([]model.Source{}, (*updated.Sources)...)
	sources = append(sources, model.Source{Name: "new", Kind: "file", Path: &addedPath, Tags: model.JSONArray[string]{}})
	updated.Sources = &sources
	if err := document.Write(path, updated, false); err != nil {
		t.Fatal(err)
	}
	written, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	for _, fragment := range []string{"article_prompt", "publish_records:", "extra_section:"} {
		if !bytes.Contains(written, []byte(fragment)) {
			t.Fatalf("missing %s: %s", fragment, written)
		}
	}
	reloaded, err := ReadIndex(path, true)
	if err != nil || reloaded.Value.Sources == nil || len(*reloaded.Value.Sources) != 2 {
		t.Fatalf("round trip = %+v, %v", reloaded, err)
	}
}
