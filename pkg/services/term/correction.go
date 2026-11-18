package term

import (
	"fmt"
	"slices"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
)

func findCorrection(term model.Term, original string) (int, error) {
	for i, c := range term.Corrections {
		if c.Original == original {
			return i, nil
		}
	}
	hint := "use `mf term correction list <TERM>` to see available corrections"
	return -1, &output.Error{Kind: "not_found", Message: fmt.Sprintf("correction \"%s\" not found on term \"%s\"", original, term.Term), Hint: &hint}
}

func (s Scope) AddCorrection(name string, correction model.Correction, dry bool) (model.Correction, bool, string, error) {
	d, err := s.document(true)
	if err != nil {
		return correction, false, "", err
	}
	i, err := findTerm(d.terms, name)
	if err != nil {
		return correction, false, "", err
	}
	for _, c := range d.terms[i].Corrections {
		if c.Original == correction.Original && c.Correct == correction.Correct {
			return c, false, "", nil
		}
	}
	warning := ""
	for _, other := range d.terms {
		if other.Term == name {
			continue
		}
		if prefixRelated(correction.Original, other.Term) || slices.ContainsFunc(other.Corrections, func(c model.Correction) bool { return prefixRelated(correction.Original, c.Original) }) {
			warning = fmt.Sprintf("original '%s' overlaps term '%s' (one is a prefix of the other); lint may map it to that term instead", correction.Original, other.Term)
			break
		}
	}
	d.terms[i].Corrections = append(d.terms[i].Corrections, correction)
	return correction, true, warning, s.write(d, dry)
}

func (s Scope) UpdateCorrection(name, original string, patch model.CorrectionPatch, dry bool) (model.Correction, error) {
	d, err := s.document(false)
	if err != nil {
		return model.Correction{}, err
	}
	i, err := findTerm(d.terms, name)
	if err != nil {
		return model.Correction{}, err
	}
	j, err := findCorrection(d.terms[i], original)
	if err != nil {
		return model.Correction{}, err
	}
	c := &d.terms[i].Corrections[j]
	if patch.Correct != nil {
		c.Correct = *patch.Correct
	}
	if patch.Match != nil {
		c.Match = *patch.Match
	}
	if patch.Fix != nil {
		c.Fix = *patch.Fix
	}
	if patch.Boundary != nil {
		c.Boundary = *patch.Boundary
	}
	if patch.Pinyin != nil {
		c.Pinyin = *patch.Pinyin
	}
	result := *c
	return result, s.write(d, dry)
}

func (s Scope) RemoveCorrection(name, original string, dry bool) (model.Correction, error) {
	d, err := s.document(false)
	if err != nil {
		return model.Correction{}, err
	}
	i, err := findTerm(d.terms, name)
	if err != nil {
		return model.Correction{}, err
	}
	j, err := findCorrection(d.terms[i], original)
	if err != nil {
		return model.Correction{}, err
	}
	c := d.terms[i].Corrections[j]
	d.terms[i].Corrections = slices.Delete(d.terms[i].Corrections, j, j+1)
	return c, s.write(d, dry)
}
