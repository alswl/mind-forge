package term

import (
	"fmt"
	"math"
	"slices"
	"strings"

	"github.com/alswl/mind-forge/pkg/common/model"
)

type Pair struct{ Term, Original string }

type Selection struct {
	Terms            []string
	Pairs            []Pair
	ExcludeTerms     []string
	ExcludeOriginals []string
	IncludeSuggested bool
	MinConfidence    *float64
	IncludeQuotes    bool
	AdHoc            []model.Correction
}

func (s Selection) Validate(terms []model.Term) error {
	if s.MinConfidence != nil {
		v := *s.MinConfidence
		if math.IsNaN(v) || math.IsInf(v, 0) || v < 0 || v > 1 {
			return fmt.Errorf("--min-confidence must be between 0.0 and 1.0")
		}
		if !s.IncludeSuggested {
			return fmt.Errorf("--min-confidence requires --include-suggested")
		}
	}
	available := map[string]model.Term{}
	for _, t := range terms {
		available[t.Term] = t
	}
	unknown := []string{}
	names := append([]string{}, s.Terms...)
	for _, p := range s.Pairs {
		names = append(names, p.Term)
	}
	for _, name := range names {
		if _, ok := available[name]; !ok && !slices.Contains(unknown, name) {
			unknown = append(unknown, name)
		}
	}
	if len(unknown) > 0 {
		slices.Sort(unknown)
		return fmt.Errorf("unknown term(s): %s", strings.Join(unknown, ", "))
	}
	for _, p := range s.Pairs {
		if !slices.ContainsFunc(available[p.Term].Corrections, func(c model.Correction) bool { return c.Original == p.Original }) {
			return fmt.Errorf("correction '%s' not found on term '%s'", p.Original, p.Term)
		}
	}
	return nil
}

func (s Selection) classify(f model.TermFinding, advisory bool) string {
	if f.QuoteProtected {
		return "quote_protected"
	}
	opted := slices.Contains(s.Terms, f.Term) || slices.Contains(s.Pairs, Pair{f.Term, f.Original})
	if !f.ReplacementEligible {
		if advisory && opted {
			return "selected"
		}
		return "ambiguous"
	}
	if f.Original == f.Correct {
		return "not_replacement"
	}
	if slices.Contains(s.ExcludeTerms, f.Term) {
		return "excluded_term"
	}
	if slices.Contains(s.ExcludeOriginals, f.Original) {
		return "excluded_original"
	}
	if (len(s.Terms) > 0 || len(s.Pairs) > 0) && !opted {
		return "not_selected"
	}
	if f.FixKind == "suggested" {
		if !s.IncludeSuggested {
			return "suggested_disabled"
		}
		confidence := 0.0
		if f.Confidence != nil {
			confidence = *f.Confidence
		}
		if s.MinConfidence != nil && confidence < *s.MinConfidence {
			return "below_confidence"
		}
	}
	return "selected"
}
