package term

import (
	"testing"

	"github.com/alswl/mind-forge/pkg/common/model"
)

func TestLegacyTermSelection(t *testing.T) {
	one, eight, below := 1.0, 0.8, 0.79
	positive := Selection{Pairs: []Pair{{"Synthetic", "wanted"}}, ExcludeOriginals: []string{"blocked"}}
	for _, test := range []struct {
		name                          string
		selection                     Selection
		term, original, fix           string
		confidence                    *float64
		eligible, advisory, protected bool
		want                          string
	}{
		{"selection_uses_inclusive_minimum_confidence", Selection{IncludeSuggested: true, MinConfidence: &eight}, "Synthetic", "old", "suggested", &eight, true, false, false, "selected"},
		{"selection_uses_inclusive_minimum_confidence", Selection{IncludeSuggested: true, MinConfidence: &eight}, "Synthetic", "old", "suggested", &below, true, false, false, "below_confidence"},
		{"selection_uses_inclusive_minimum_confidence", Selection{IncludeSuggested: true, MinConfidence: &eight}, "Synthetic", "old", "suggested", nil, true, false, false, "below_confidence"},
		{"selection_excluded_term_overrides_positive_scope", Selection{Terms: []string{"Synthetic"}, ExcludeTerms: []string{"Synthetic"}}, "Synthetic", "old", "required", &one, true, false, false, "excluded_term"},
		{"selection_exact_pair_and_original_exclusion_are_independent", positive, "Synthetic", "wanted", "required", &one, true, false, false, "selected"},
		{"selection_exact_pair_and_original_exclusion_are_independent", positive, "Synthetic", "other", "required", &one, true, false, false, "not_selected"},
		{"selection_exact_pair_and_original_exclusion_are_independent", positive, "Other", "blocked", "required", &one, true, false, false, "excluded_original"},
		{"advisory_finding_requires_explicit_opt_in", Selection{}, "Synthetic", "以可", "required", &one, false, true, false, "ambiguous"},
		{"advisory_finding_requires_explicit_opt_in", Selection{Pairs: []Pair{{"Synthetic", "以可"}}}, "Synthetic", "以可", "required", &one, false, true, false, "selected"},
		{"advisory_finding_requires_explicit_opt_in", Selection{Terms: []string{"Synthetic"}}, "Synthetic", "ambiguous", "required", &one, false, false, false, "ambiguous"},
		{"quote_protected_finding_is_never_selected_even_when_named", Selection{}, "Synthetic", "old", "required", &one, true, false, true, "quote_protected"},
		{"quote_protected_finding_is_never_selected_even_when_named", Selection{Terms: []string{"Synthetic"}}, "Synthetic", "old", "required", &one, true, false, true, "quote_protected"},
		{"quote_protected_finding_is_never_selected_even_when_named", Selection{Pairs: []Pair{{"Synthetic", "old"}}}, "Synthetic", "old", "required", &one, true, false, true, "quote_protected"},
		{"quote_protected_takes_priority_over_advisory_classification", Selection{}, "Synthetic", "以可", "required", &one, false, true, true, "quote_protected"},
	} {
		t.Run(test.name, func(t *testing.T) {
			f := model.TermFinding{Term: test.term, Original: test.original, Correct: "new", Confidence: test.confidence, FixKind: test.fix, ReplacementEligible: test.eligible, QuoteProtected: test.protected}
			if got := test.selection.classify(f, test.advisory); got != test.want {
				t.Fatalf("selection = %q, want %q", got, test.want)
			}
		})
	}
}
