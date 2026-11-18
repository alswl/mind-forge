package term

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/alswl/mind-forge/pkg/common/model"
)

const legacyGlobalSubstring = "schema_version: '1'\nterms:\n- term: AIDC\n  corrections:\n  - original: aidc\n    correct: AIDC\n    match: substring\n"

func TestLegacyGlobalTermFormat(t *testing.T) {
	for _, test := range []struct {
		name, body                             string
		strict                                 bool
		wantMatch, wantBoundary, errorFragment string
	}{
		{"global_strict_load_accepts_supported_substring", legacyGlobalSubstring, true, "substring", "standalone", ""},
		{"global_lenient_load_keeps_supported_substring_without_rewrite", legacyGlobalSubstring, false, "substring", "", ""},
		{"global_load_allows_pinyin_with_standalone", "schema_version: '1'\nterms:\n- term: SYNTHETIC\n  corrections:\n  - original: synthetic\n    correct: synthetic\n    match: pinyin\n    boundary: standalone\n", true, "", "", ""},
		{"global_load_rejects_standalone_edge_hyphen", "schema_version: '1'\nterms:\n- term: AIDC\n  corrections:\n  - original: aidc-\n    correct: AIDC\n    boundary: standalone\n", true, "", "", "identifier-character edges"},
		{"global_load_accepts_valid_corrections", "schema_version: '1'\nterms:\n- term: AIDC\n  corrections:\n  - original: aidc\n    correct: AIDC\n    boundary: standalone\n", true, "", "", ""},
	} {
		t.Run(test.name, func(t *testing.T) {
			scope := Scope{Root: t.TempDir(), Kind: "global"}
			if err := os.WriteFile(scope.Path(), []byte(test.body), 0600); err != nil {
				t.Fatal(err)
			}
			got, err := scope.Load(test.strict)
			if test.errorFragment != "" {
				if err == nil || !strings.Contains(err.Error(), test.errorFragment) {
					t.Fatalf("global load = %+v, %v", got, err)
				}
				return
			}
			if err != nil || len(got) != 1 || len(got[0].Corrections) != 1 {
				t.Fatalf("global load = %+v, %v", got, err)
			}
			correction := got[0].Corrections[0]
			if test.wantMatch != "" && correction.Match != test.wantMatch {
				t.Fatalf("match = %s", correction.Match)
			}
			if test.wantBoundary != "" && correction.Boundary != test.wantBoundary {
				t.Fatalf("boundary = %s", correction.Boundary)
			}
			raw, err := os.ReadFile(scope.Path())
			if err != nil || string(raw) != test.body {
				t.Fatalf("load rewrote bytes = %q, %v", raw, err)
			}
		})
	}
}

func TestGlobalPinyinStandaloneIgnoresIdentifierEdges(t *testing.T) {
	scope := Scope{Root: t.TempDir(), Kind: "global"}
	body := "schema_version: '1'\nterms:\n- term: SYNTHETIC\n  corrections:\n  - {original: synthetic-, correct: Synthetic, match: pinyin, boundary: standalone}\n"
	if err := os.WriteFile(scope.Path(), []byte(body), 0600); err != nil {
		t.Fatal(err)
	}
	if _, err := scope.Load(true); err != nil {
		t.Fatalf("pinyin boundary should be ignored: %v", err)
	}
}

func TestLegacyNoImplicitHomophonePipeline(t *testing.T) {
	project := t.TempDir()
	if err := os.WriteFile(filepath.Join(project, "synthetic.md"), []byte("实刻开始工作\n"), 0600); err != nil {
		t.Fatal(err)
	}
	confidence := 1.0
	terms := []model.Term{{Term: "时刻", Confidence: &confidence}}
	got, err := LintFiles(project, filepath.Join(project, "synthetic.md"), terms, LintOptions{})
	if err != nil || len(got.Findings) != 0 {
		t.Fatalf("canonical-only term synthesized fixes: %+v, %v", got, err)
	}
}
