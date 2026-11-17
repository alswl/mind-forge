package term

import (
	"errors"
	"os"
	"testing"

	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
)

func TestMissingTermUpdateRetainsScopeSpecificError(t *testing.T) {
	for _, kind := range []string{"project", "global"} {
		t.Run(kind, func(t *testing.T) {
			scope := Scope{Root: t.TempDir(), Kind: kind}
			original := []byte("schema: '1'\nterms: []\n")
			if err := os.WriteFile(scope.Path(), original, 0600); err != nil {
				t.Fatal(err)
			}
			for _, patch := range []Patch{{Tags: []string{"new"}}, {CorrectionMatches: []string{"missing:word"}}} {
				_, err := scope.Update("Absent", patch, false)
				var failure *output.Error
				wantKind, wantExit := "not_found", 1
				if kind == "global" {
					wantKind, wantExit = "usage", 2
				}
				if !errors.As(err, &failure) || failure.Kind != wantKind || failure.ExitCode() != wantExit || failure.Message != "term 'Absent' not found" || failure.Hint == nil || *failure.Hint != "use `mf term list` or `mf term new`" {
					t.Fatalf("missing %s update = %#v, want %s/exit%d", kind, failure, wantKind, wantExit)
				}
				current, readError := os.ReadFile(scope.Path())
				if readError != nil || string(current) != string(original) {
					t.Fatal("rejected update changed terms", readError)
				}
			}
			_, err := scope.Show("Absent")
			var failure *output.Error
			if !errors.As(err, &failure) || failure.Kind != "not_found" || failure.ExitCode() != 1 {
				t.Fatalf("show category changed: %v", err)
			}
		})
	}
}

func legacySyntheticScope(t *testing.T) Scope {
	t.Helper()
	scope := Scope{Root: t.TempDir(), Kind: "project"}
	body := "schema: '1'\nterms:\n- term: Synthetic\n  definition: unchanged\n  corrections:\n  - {original: first, correct: Synthetic, match: word, fix: required, boundary: standalone}\n  - {original: second, correct: Synthetic, match: word, fix: required, boundary: standalone}\n"
	if err := os.WriteFile(scope.Path(), []byte(body), 0600); err != nil {
		t.Fatal(err)
	}
	return scope
}

func TestLegacyTermCorrectionUpdates(t *testing.T) {
	t.Run("update_sets_exact_correction_match_without_touching_siblings", func(t *testing.T) {
		got, err := legacySyntheticScope(t).Update("Synthetic", Patch{CorrectionMatches: []string{"first:pinyin"}}, false)
		if err != nil {
			t.Fatal(err)
		}
		if len(got.Corrections) != 2 || got.Corrections[0].Match != "pinyin" || got.Corrections[1].Match != "word" || got.Definition == nil || *got.Definition != "unchanged" {
			t.Fatalf("updated = %+v", got)
		}
	})
	t.Run("update_pinyin_and_delete_target_exact_original_only", func(t *testing.T) {
		got, err := legacySyntheticScope(t).Update("Synthetic", Patch{CorrectionPinyins: []string{"first:synthetic-reading"}, DeleteCorrections: []string{"second"}}, false)
		if err != nil {
			t.Fatal(err)
		}
		if len(got.Corrections) != 1 || got.Corrections[0].Original != "first" || got.Corrections[0].Pinyin == nil || *got.Corrections[0].Pinyin != "synthetic-reading" {
			t.Fatalf("updated = %+v", got)
		}
	})
	t.Run("correction_match_to_pinyin_resets_standalone_boundary", func(t *testing.T) {
		scope := legacySyntheticScope(t)
		before, err := scope.Show("Synthetic")
		if err != nil {
			t.Fatal(err)
		}
		if before.Corrections[0].Boundary != "standalone" {
			t.Fatalf("seed boundary = %q", before.Corrections[0].Boundary)
		}
		got, err := scope.Update("Synthetic", Patch{CorrectionMatches: []string{"first:pinyin"}}, false)
		if err != nil {
			t.Fatal(err)
		}
		if got.Corrections[0].Match != "pinyin" || got.Corrections[0].Boundary != "loose" {
			t.Fatalf("normalized = %+v", got.Corrections[0])
		}
		if err := model.ValidateCorrections([]model.Term{got}); err != nil {
			t.Fatal(err)
		}
	})
	t.Run("add_correction_sets_correct_text_and_defaults_to_term_name", func(t *testing.T) {
		got, err := legacySyntheticScope(t).Update("Synthetic", Patch{AddCorrections: []string{"typo:Synthetic", "bare"}}, false)
		if err != nil {
			t.Fatal(err)
		}
		for _, original := range []string{"typo", "bare"} {
			index, err := findCorrection(got, original)
			if err != nil {
				t.Fatal(err)
			}
			if got.Corrections[index].Correct != "Synthetic" {
				t.Fatalf("correct(%s) = %q", original, got.Corrections[index].Correct)
			}
		}
	})
}

func TestCorrectionUpdateCanSelectSecondSibling(t *testing.T) {
	got, err := legacySyntheticScope(t).Update("Synthetic", Patch{CorrectionMatches: []string{"second:pinyin"}}, false)
	if err != nil {
		t.Fatal(err)
	}
	if len(got.Corrections) != 2 || got.Corrections[0].Match != "word" || got.Corrections[1].Match != "pinyin" {
		t.Fatalf("second-target update = %+v", got.Corrections)
	}
}
