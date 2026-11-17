package cli

import (
	"errors"
	"fmt"
	"io"

	"github.com/alswl/mind-forge/internal/app"
	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/internal/term"
	"github.com/spf13/cobra"
)

func termUpdateCommand(c *cobra.Command, a *app.Context, name string, stdout, stderr io.Writer) error {
	if len(termArray(c, "misrecognition")) > 0 {
		return output.Usage("--misrecognition is not supported on `term update`; use `mf term correction add` to add a correction to an existing term, or `mf term new --misrecognition` when creating a new term", "use `mf term correction add <TERM> <ORIGINAL> <CORRECT>` to add a correction")
	}
	p := term.Patch{Definition: optionalFlag(c, "definition"), Description: optionalFlag(c, "description"), ClearDescription: boolFlag(c, "clear-description"), Confidence: termFloat(c, "confidence"), ClearConfidence: boolFlag(c, "clear-confidence"), Aliases: termArray(c, "alias"), Tags: termArray(c, "tag"), DeleteAliases: termArray(c, "delete-alias"), DeleteTags: termArray(c, "delete-tag"), AddCorrections: termArray(c, "add-correction"), DeleteCorrections: termArray(c, "delete-correction"), CorrectionMatches: termArray(c, "correction-match"), CorrectionFixes: termArray(c, "correction-fix"), CorrectionPinyins: termArray(c, "correction-pinyin")}
	if p.Description != nil && p.ClearDescription {
		return output.Usage("--description and --clear-description are mutually exclusive", "")
	}
	if p.Confidence != nil && p.ClearConfidence {
		return output.Usage("--confidence and --clear-confidence are mutually exclusive", "")
	}
	result := output.VerbResult{Verb: output.Update, Kind: "term", Identity: name, DryRun: boolFlag(c, "dry-run"), Details: map[string]any{}}
	if result.DryRun {
		_, scope, warnings, err := termFallback(c, a, name, func(s term.Scope) (model.Term, error) { return s.Show(name) })
		if err != nil {
			return err
		}
		planned := []string{}
		if p.Definition != nil {
			planned = append(planned, "definition")
		}
		if p.Description != nil || p.ClearDescription {
			planned = append(planned, "description")
		}
		if p.Confidence != nil || p.ClearConfidence {
			planned = append(planned, "confidence")
		}
		for _, v := range []struct {
			items []string
			label string
		}{{p.Aliases, "add %d alias(es)"}, {p.Tags, "add %d tag(s)"}, {p.DeleteAliases, "remove %d alias(es)"}, {p.DeleteTags, "remove %d tag(s)"}, {p.AddCorrections, "add %d correction(s)"}, {p.DeleteCorrections, "remove %d correction(s)"}, {p.CorrectionMatches, "set match kind on %d correction(s)"}, {p.CorrectionFixes, "set fix kind on %d correction(s)"}, {p.CorrectionPinyins, "set pinyin on %d correction(s)"}} {
			if len(v.items) > 0 {
				planned = append(planned, fmt.Sprintf(v.label, len(v.items)))
			}
		}
		result.Details = map[string]any{"scope": scope, "changes": planned}
		return termVerb(a, stdout, stderr, result, warnings)
	}
	_, scope, warnings, err := termFallback(c, a, name, func(s term.Scope) (model.Term, error) {
		tm, err := s.Update(name, p, false)
		return tm, termUpdateError(s.Kind, err)
	})
	if err != nil {
		return err
	}
	changes := map[string]any{}
	for _, v := range []struct {
		changed bool
		name    string
	}{{p.Definition != nil, "definition"}, {p.Description != nil || p.ClearDescription, "description"}, {p.Confidence != nil || p.ClearConfidence, "confidence"}} {
		if v.changed {
			changes[v.name] = map[string]any{"changed": true}
		}
	}
	for _, v := range []struct {
		items        []string
		name, action string
	}{{p.Aliases, "aliases", "added"}, {p.DeleteAliases, "aliases", "deleted"}, {p.Tags, "tags", "added"}, {p.DeleteTags, "tags", "deleted"}} {
		if len(v.items) > 0 {
			changes[v.name] = map[string]any{v.action: v.items}
		}
	}
	corrections := map[string]any{}
	if len(p.AddCorrections) > 0 {
		corrections["added"] = p.AddCorrections
	}
	if len(p.DeleteCorrections) > 0 {
		corrections["deleted"] = p.DeleteCorrections
	}
	if len(corrections) > 0 {
		changes["corrections"] = corrections
	}
	for _, v := range []struct {
		items []string
		name  string
	}{{p.CorrectionMatches, "correction_match"}, {p.CorrectionFixes, "correction_fix"}, {p.CorrectionPinyins, "correction_pinyin"}} {
		if len(v.items) > 0 {
			changes[v.name] = v.items
		}
	}
	result.Details["changes"] = changes
	if len(warnings) > 0 {
		result.Details["scope"] = scope
	}
	return termVerb(a, stdout, stderr, result, warnings)
}

func termUpdateError(scope string, err error) error {
	var failure *output.Error
	if scope == "project" && !term.IsMissingTerm(err) && errors.As(err, &failure) && failure.Kind == "not_found" {
		usage := *failure
		usage.Kind = "usage"
		return &usage
	}
	return err
}
