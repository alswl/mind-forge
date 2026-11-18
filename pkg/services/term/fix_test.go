package term

import (
	"reflect"
	"testing"
)

func TestLegacyFixSpans(t *testing.T) {
	span := func(start, end int, text string, order int) Replacement {
		return Replacement{Start: start, End: end, Text: text, Order: order}
	}
	for _, test := range []struct {
		name    string
		content string
		spans   []Replacement
		chosen  []Replacement
		output  string
	}{
		{"apply_single_fix", "hello world", []Replacement{span(6, 11, "there", 0)}, []Replacement{span(6, 11, "there", 0)}, "hello there"},
		{"apply_multiple_fixes", "aa bb aa", []Replacement{span(0, 2, "x", 0), span(6, 8, "y", 1)}, []Replacement{span(0, 2, "x", 0), span(6, 8, "y", 1)}, "x bb y"},
		{"overlapping_spans_first_wins", "mini pass", []Replacement{span(0, 9, "mini PaaS", 0), span(5, 9, "PaaS", 1)}, []Replacement{span(0, 9, "mini PaaS", 0)}, "mini PaaS"},
		{"overlapping_spans_apply_fixes_skips_nested", "mini pass end", []Replacement{span(0, 9, "mini PaaS", 0), span(5, 9, "PaaS", 1)}, []Replacement{span(0, 9, "mini PaaS", 0)}, "mini PaaS end"},
		{"deduplicate_spans_non_overlapping_kept", "aa bb cc", []Replacement{span(0, 2, "X", 0), span(5, 7, "Y", 1)}, []Replacement{span(0, 2, "X", 0), span(5, 7, "Y", 1)}, "X bbYc"},
		{"deduplicate_spans_empty", "unchanged", []Replacement{}, []Replacement{}, "unchanged"},
		{"same_start_longest_end_wins_even_when_declared_later", "XXX 3.0", []Replacement{span(0, 1, "AIDC", 0), span(0, 3, "AIDC-Long", 1)}, []Replacement{span(0, 3, "AIDC-Long", 1)}, "AIDC-Long 3.0"},
		{"same_start_longest_end_wins_when_declared_first", "XXX 3.0", []Replacement{span(0, 3, "AIDC-Long", 0), span(0, 1, "AIDC", 1)}, []Replacement{span(0, 3, "AIDC-Long", 0)}, "AIDC-Long 3.0"},
		{"same_start_same_end_earlier_declaration_wins", "XXXX", []Replacement{span(0, 4, "B-WINS", 5), span(0, 4, "A-WINS", 2)}, []Replacement{span(0, 4, "A-WINS", 2)}, "A-WINS"},
		{"cross_rule_overlap_keeps_earliest_then_longest", "XXX YYY", []Replacement{span(0, 3, "AIDC-Long", 0), span(1, 4, "DROPPED", 1), span(4, 7, "NEXT", 2)}, []Replacement{span(0, 3, "AIDC-Long", 0), span(4, 7, "NEXT", 2)}, "AIDC-Long NEXT"},
		{"no_panic_on_overlapping_candidates_regression_1c05809", "mini pass", []Replacement{span(5, 9, "PaaS", 0), span(0, 9, "mini PaaS", 1)}, []Replacement{span(0, 9, "mini PaaS", 1)}, "mini PaaS"},
	} {
		t.Run(test.name, func(t *testing.T) {
			if got := chosenSpans(test.content, test.spans); !reflect.DeepEqual(got, test.chosen) {
				t.Fatalf("selected spans = %+v, want %+v", got, test.chosen)
			}
			if got := Apply(test.content, test.spans); got != test.output {
				t.Fatalf("applied bytes = %q, want %q", got, test.output)
			}
		})
	}
}
