package term

import (
	"os"
	"strings"
	"testing"

	"github.com/alswl/mind-forge/pkg/common/model"
)

func legacyShadowingWarning(t *testing.T, termName, original string) string {
	t.Helper()
	scope := Scope{Root: t.TempDir(), Kind: "project"}
	body := "schema: '1'\nterms:\n- {term: " + termName + "}\n- {term: gateway}\n"
	if err := os.WriteFile(scope.Path(), []byte(body), 0600); err != nil {
		t.Fatal(err)
	}
	_, _, warning, err := scope.AddCorrection("gateway", model.Correction{Original: original, Correct: original + "-fixed"}, false)
	if err != nil {
		t.Fatal(err)
	}
	return warning
}

func TestLegacyCorrectionShadowing(t *testing.T) {
	t.Run("shadowing_conflict_is_found_in_both_prefix_directions", func(t *testing.T) {
		for _, test := range []struct{ term, original string }{{"API", "API网关"}, {"API网关", "API"}} {
			got := legacyShadowingWarning(t, test.term, test.original)
			if !strings.Contains(got, "term '"+test.term+"'") {
				t.Fatalf("conflict = %q, want %q", got, test.term)
			}
		}
	})
	t.Run("shadowing_warning_does_not_claim_a_direction_it_did_not_check", func(t *testing.T) {
		got := legacyShadowingWarning(t, "API", "API网关")
		if !strings.Contains(got, "API网关") || !strings.Contains(got, "term 'API'") || strings.Contains(got, "is also a prefix of term") || !strings.Contains(got, "one is a prefix of the other") {
			t.Fatalf("warning = %q", got)
		}
	})
	t.Run("the_term_being_edited_is_never_its_own_conflict", func(t *testing.T) {
		scope := Scope{Root: t.TempDir(), Kind: "project"}
		if err := os.WriteFile(scope.Path(), []byte("schema: '1'\nterms:\n- term: API\n  corrections:\n  - {original: API网关, correct: API网关-fixed}\n"), 0600); err != nil {
			t.Fatal(err)
		}
		_, _, warning, err := scope.AddCorrection("API", model.Correction{Original: "API网关", Correct: "replacement"}, false)
		if err != nil || warning != "" {
			t.Fatalf("self conflict = %q, %v", warning, err)
		}
	})
}
