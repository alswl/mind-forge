package main

import (
	"fmt"
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("term correction remove", "term correction", newTermCorrectionRemoveCommand, func(e commandExecution) error {
		return executeTermCorrectionRemove(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newTermCorrectionRemoveCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "remove"}
	return configureCommand(command, item, handler)
}

func executeTermCorrectionRemove(c *cobra.Command, a *app.Context, args []string, stdout, stderr io.Writer) error {

	return executeTermCorrectionRemoveDetails(c, a, args, stdout, stderr)
}

func executeTermCorrectionRemoveDetails(c *cobra.Command, a *app.Context, args []string, stdout, stderr io.Writer) error {
	scope, err := termScope(c, a)
	if err != nil {
		return err
	}
	name := args[0]
	dry := boolFlag(c, "dry-run")
	if dry {
		return termVerb(a, stdout, stderr, output.VerbResult{Verb: output.Remove, Kind: "correction", Identity: name + "/" + args[1], DryRun: true, Details: map[string]any{"scope": scope.Kind}}, nil)
	}

	{

		corr, err := scope.RemoveCorrection(name, args[1], false)
		if err != nil {
			return err
		}
		return renderTerm(a, stdout, stderr, map[string]any{"term": name, "scope": scope.Kind, "removed": corr}, fmt.Sprintf("removed correction \"%s\" → \"%s\" from term \"%s\"", corr.Original, corr.Correct, name), nil)
	}
}
