package term

import (
	"fmt"
	"slices"
	"strings"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
	"github.com/alswl/mind-forge/pkg/common/textcase"
)

type Patch struct {
	Definition, Description                                                                  *string
	ClearDescription, ClearConfidence                                                        bool
	Confidence                                                                               *float64
	Aliases, Tags, DeleteAliases, DeleteTags                                                 []string
	AddCorrections, DeleteCorrections, CorrectionMatches, CorrectionFixes, CorrectionPinyins []string
}

func (s Scope) Update(name string, patch Patch, dry bool) (model.Term, error) {
	if patch.Description != nil && patch.ClearDescription {
		return model.Term{}, output.Usage("--description and --clear-description are mutually exclusive", "")
	}
	if patch.Confidence != nil && patch.ClearConfidence {
		return model.Term{}, output.Usage("--confidence and --clear-confidence are mutually exclusive", "")
	}
	if err := validateConfidence(patch.Confidence); err != nil {
		return model.Term{}, err
	}
	if patch.Definition == nil && patch.Description == nil && !patch.ClearDescription && patch.Confidence == nil && !patch.ClearConfidence && len(patch.Aliases)+len(patch.Tags)+len(patch.DeleteAliases)+len(patch.DeleteTags)+len(patch.AddCorrections)+len(patch.DeleteCorrections)+len(patch.CorrectionMatches)+len(patch.CorrectionFixes)+len(patch.CorrectionPinyins) == 0 {
		return model.Term{}, output.Usage("at least one of --definition, --description, --confidence, --alias, --tag, --delete-alias, --delete-tag, --clear-description, --clear-confidence, --add-correction, --delete-correction, --correction-match, --correction-fix, --correction-pinyin must be provided", "")
	}
	d, err := s.document(false)
	if err != nil {
		return model.Term{}, err
	}
	i, err := findTerm(d.terms, name)
	if err != nil {
		if s.Kind == "global" {
			return model.Term{}, output.Usage(err.Error(), "use `mf term list` or `mf term new`")
		}
		return model.Term{}, err
	}
	count := 0
	for _, tm := range d.terms {
		if tm.Term == name {
			count++
		}
	}
	if count > 1 {
		return model.Term{}, &output.Error{Kind: "incompatible_schema", Message: "duplicate term '" + name + "'"}
	}
	tm := &d.terms[i]
	if patch.Definition != nil {
		tm.Definition = patch.Definition
		if *patch.Definition == "" {
			tm.Definition = nil
		}
	}
	if patch.ClearDescription {
		tm.Description = nil
	} else if patch.Description != nil {
		tm.Description = patch.Description
		if *patch.Description == "" {
			tm.Description = nil
		}
	}
	if patch.ClearConfidence {
		tm.Confidence = nil
	} else if patch.Confidence != nil {
		tm.Confidence = patch.Confidence
	}
	for _, alias := range patch.Aliases {
		if !slices.Contains(tm.Aliases, alias) {
			tm.Aliases = append(tm.Aliases, alias)
		}
	}
	tm.Aliases = slices.DeleteFunc(tm.Aliases, func(a string) bool { return slices.Contains(patch.DeleteAliases, a) })
	for _, tag := range patch.Tags {
		if !slices.Contains(tm.Tags, tag) {
			tm.Tags = append(tm.Tags, tag)
		}
	}
	tm.Tags = slices.DeleteFunc(tm.Tags, func(a string) bool { return slices.Contains(patch.DeleteTags, a) })
	for _, raw := range patch.AddCorrections {
		original, correct, ok := strings.Cut(raw, ":")
		if !ok || correct == "" {
			original = raw
			correct = name
		}
		if !slices.ContainsFunc(tm.Corrections, func(c model.Correction) bool { return c.Original == original }) {
			tm.Corrections = append(tm.Corrections, model.Correction{Original: original, Correct: correct})
		}
	}
	for _, original := range patch.DeleteCorrections {
		j, err := findCorrection(*tm, original)
		if err != nil {
			return model.Term{}, err
		}
		tm.Corrections = slices.Delete(tm.Corrections, j, j+1)
	}
	for _, attr := range []struct {
		flag   string
		values []string
	}{{"correction-match", patch.CorrectionMatches}, {"correction-fix", patch.CorrectionFixes}, {"correction-pinyin", patch.CorrectionPinyins}} {
		for _, raw := range attr.values {
			original, value, ok := strings.Cut(raw, ":")
			if !ok {
				return model.Term{}, output.Usage(fmt.Sprintf("invalid %s '%s'; expected ORIGINAL:VALUE", attr.flag, raw), "")
			}
			if original == "" {
				return model.Term{}, output.Usage(attr.flag+" ORIGINAL cannot be empty in '"+raw+"'", "")
			}
			j, err := findCorrection(*tm, original)
			if err != nil {
				return model.Term{}, err
			}
			c := &tm.Corrections[j]
			switch attr.flag {
			case "correction-match":
				value = textcase.ASCIILower(value)
				if value != "word" && value != "substring" && value != "pinyin" {
					return model.Term{}, output.Usage("invalid match kind '"+value+"'", "")
				}
				c.Match = value
				if value == "pinyin" && c.Boundary != "loose" {
					c.Boundary = "loose"
				}
			case "correction-fix":
				value = textcase.ASCIILower(value)
				if value != "required" && value != "suggested" {
					return model.Term{}, output.Usage("invalid fix kind '"+value+"'", "")
				}
				c.Fix = value
			case "correction-pinyin":
				c.Pinyin = nil
				if value != "" {
					c.Pinyin = stringPtr(value)
				}
			}
		}
	}
	result := *tm
	return result, s.write(d, dry)
}
