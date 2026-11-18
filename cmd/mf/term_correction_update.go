package main

import (
	"fmt"
	"io"

	"github.com/alswl/mind-forge/pkg/common/model"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("term correction update", "term correction", newTermCorrectionUpdateCommand, func(e commandExecution) error {
		return executeTermCorrectionUpdate(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newTermCorrectionUpdateCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "update"}
	return configureCommand(command, item, handler)
}

func executeTermCorrectionUpdate(c *cobra.Command, a *app.Context, args []string, stdout, stderr io.Writer) error {

	return executeTermCorrectionUpdateDetails(c, a, args, stdout, stderr)
}

func executeTermCorrectionUpdateDetails(c *cobra.Command, a *app.Context, args []string, stdout, stderr io.Writer) error {
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
	{

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
}
