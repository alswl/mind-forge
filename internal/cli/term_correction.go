package cli

import (
	"fmt"
	"io"
	"strings"

	"github.com/alswl/mind-forge/internal/app"
	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/spf13/cobra"
)

func correctionOption(c *cobra.Command, name string, allowed ...string) (*string, error) {
	v := optionalFlag(c, name)
	if v == nil {
		return nil, nil
	}
	lower := strings.ToLower(*v)
	for _, known := range allowed {
		if lower == known {
			return &lower, nil
		}
	}
	label := name
	if name == "match" || name == "fix" {
		label += " kind"
	}
	return nil, output.Usage(fmt.Sprintf("invalid %s '%s'; expected %s", label, lower, strings.Join(allowed, ", ")), "")
}

func correctionText(c model.Correction) (match, fix, boundary string) {
	match, fix, boundary = c.Match, c.Fix, c.Boundary
	if match == "" {
		match = "word"
	}
	if fix == "" {
		fix = "required"
	}
	if boundary == "" {
		boundary = "standalone"
	}
	return
}

func termCorrectionCommand(c *cobra.Command, a *app.Context, args []string, stdout, stderr io.Writer) error {
	scope, err := termScope(c, a)
	if err != nil {
		return err
	}
	name := args[0]
	dry := boolFlag(c, "dry-run")
	if c.Name() == "remove" && dry {
		return termVerb(a, stdout, stderr, output.VerbResult{Verb: output.Remove, Kind: "correction", Identity: name + "/" + args[1], DryRun: true, Details: map[string]any{"scope": scope.Kind}}, nil)
	}
	if c.Name() == "list" || c.Name() == "show" {
		terms, err := scope.Load(true)
		if err != nil {
			return err
		}
		var tm *model.Term
		for i := range terms {
			if terms[i].Term == name {
				tm = &terms[i]
				break
			}
		}
		if tm == nil {
			return &output.Error{Kind: "not_found", Message: "term '" + name + "' not found"}
		}
		if c.Name() == "list" {
			lines := []string{}
			if len(tm.Corrections) == 0 {
				lines = append(lines, fmt.Sprintf("no corrections for term \"%s\"", name))
			} else {
				lines = append(lines, fmt.Sprintf("corrections for term \"%s\":", name))
				for _, corr := range tm.Corrections {
					match, fix, _ := correctionText(corr)
					lines = append(lines, fmt.Sprintf("  \"%s\" → \"%s\" [match=%s, fix=%s]", corr.Original, corr.Correct, match, fix))
				}
			}
			return renderTerm(a, stdout, stderr, map[string]any{"term": name, "scope": scope.Kind, "corrections": tm.Corrections}, strings.Join(lines, "\n"), nil)
		}
		for _, corr := range tm.Corrections {
			if corr.Original == args[1] {
				match, fix, boundary := correctionText(corr)
				text := fmt.Sprintf("correction \"%s\" → \"%s\"\n  match: %s\n  fix: %s\n  boundary: %s", corr.Original, corr.Correct, match, fix, boundary)
				if corr.Pinyin != nil {
					text += "\n  pinyin: " + *corr.Pinyin
				}
				return renderTerm(a, stdout, stderr, map[string]any{"term": name, "scope": scope.Kind, "correction": corr}, text, nil)
			}
		}
		return &output.Error{Kind: "not_found", Message: fmt.Sprintf("correction \"%s\" not found on term \"%s\"", args[1], name)}
	}
	if c.Name() == "remove" {
		corr, err := scope.RemoveCorrection(name, args[1], false)
		if err != nil {
			return err
		}
		return renderTerm(a, stdout, stderr, map[string]any{"term": name, "scope": scope.Kind, "removed": corr}, fmt.Sprintf("removed correction \"%s\" → \"%s\" from term \"%s\"", corr.Original, corr.Correct, name), nil)
	}
	match, err := correctionOption(c, "match", "word", "substring", "pinyin")
	if err != nil {
		return err
	}
	fix, err := correctionOption(c, "fix", "required", "suggested")
	if err != nil {
		return err
	}
	boundary, err := correctionOption(c, "boundary", "loose", "standalone")
	if err != nil {
		return err
	}
	var pinyin **string
	if v := optionalFlag(c, "pinyin"); v != nil {
		if *v == "" {
			v = nil
		}
		pinyin = &v
	}
	if c.Name() == "update" {
		corr, err := scope.UpdateCorrection(name, args[1], model.CorrectionPatch{Correct: optionalFlag(c, "correct"), Match: match, Fix: fix, Boundary: boundary, Pinyin: pinyin}, dry)
		if err != nil {
			return err
		}
		action := "updated"
		if dry {
			action = "would update"
		}
		return renderTerm(a, stdout, stderr, map[string]any{"term": name, "scope": scope.Kind, "dry_run": dry, "correction": corr}, fmt.Sprintf("%s correction \"%s\" on term \"%s\"", action, args[1], name), nil)
	}
	corr := model.Correction{Original: args[1], Correct: args[2]}
	if match != nil {
		corr.Match = *match
	}
	if fix != nil {
		corr.Fix = *fix
	}
	if boundary != nil {
		corr.Boundary = *boundary
	}
	if pinyin != nil {
		corr.Pinyin = *pinyin
	}
	corr, created, warning, err := scope.AddCorrection(name, corr, dry)
	if err != nil {
		return err
	}
	warnings := []string{}
	if warning != "" {
		warnings = append(warnings, warning)
	}
	data := map[string]any{"term": name, "scope": scope.Kind, "created": created, "correction": corr}
	if dry {
		return termVerb(a, stdout, stderr, output.VerbResult{Verb: output.Add, Kind: "term_correction", Identity: name + "::" + corr.Original, DryRun: true, Details: data}, warnings)
	}
	text := fmt.Sprintf("added correction \"%s\" → \"%s\" to term \"%s\"", corr.Original, corr.Correct, name)
	if !created {
		text = fmt.Sprintf("correction \"%s\" → \"%s\" already exists on term \"%s\", skipped", corr.Original, corr.Correct, name)
	}
	return renderTerm(a, stdout, stderr, data, text, warnings)
}
