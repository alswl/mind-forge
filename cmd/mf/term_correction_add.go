package main

import (
	"fmt"
	"io"

	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/pkg/common/model"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("term correction add", "term correction", newTermCorrectionAddCommand, func(e commandExecution) error {
		return executeTermCorrectionAdd(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newTermCorrectionAddCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "add"}
	return configureCommand(command, item, handler)
}

func executeTermCorrectionAdd(c *cobra.Command, a *app.Context, args []string, stdout, stderr io.Writer) error {

	return executeTermCorrectionAddDetails(c, a, args, stdout, stderr)
}

func executeTermCorrectionAddDetails(c *cobra.Command, a *app.Context, args []string, stdout, stderr io.Writer) error {
	scope, err := termScope(c, a)
	if err != nil {
		return err
	}
	name := args[0]
	dry := boolFlag(c, "dry-run")

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
		result := output.VerbResult{Verb: output.Add, Kind: "term_correction", Identity: name + "::" + corr.Original, DryRun: true, Details: data}
		profile := commandProfile(stdout, a)
		return renderCorrection(a, stdout, stderr, output.VerbEnvelope(result), output.RenderVerb(result, output.DisplayOptions{RepoRoot: a.RepoRoot, Hyperlinks: profile.Hyperlinks}), warnings)
	}
	text := fmt.Sprintf("added correction \"%s\" → \"%s\" to term \"%s\"", corr.Original, corr.Correct, name)
	if !created {
		text = fmt.Sprintf("correction \"%s\" → \"%s\" already exists on term \"%s\", skipped", corr.Original, corr.Correct, name)
	}
	return renderCorrection(a, stdout, stderr, data, text, warnings)
}
