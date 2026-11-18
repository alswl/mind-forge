package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/term"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("term remove", "term", newTermRemoveCommand, func(e commandExecution) error {
		return executeTermRemove(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newTermRemoveCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "remove"}
	return configureCommand(command, item, handler)
}

func executeTermRemove(c *cobra.Command, a *app.Context, args []string, stdout, stderr io.Writer) error {

	return executeTermRemoveDetails(c, a, args, stdout, stderr)
}

func executeTermRemoveDetails(c *cobra.Command, a *app.Context, args []string, stdout, stderr io.Writer) error {
	dry, force := boolFlag(c, "dry-run"), boolFlag(c, "force")
	result := output.VerbResult{Kind: "term", Identity: args[0], DryRun: dry, Details: map[string]any{}}

	if err := output.RequireConfirmation(output.ConfirmArgs{VerbLabel: "removal", Kind: "term", Identity: args[0], Yes: boolFlag(c, "yes"), Force: force}, stderr, nil); err != nil {
		return err
	}
	_, _, warnings, err := termFallback(c, a, args[0], func(s term.Scope) (bool, error) { return true, s.Remove(args[0], force, dry) })
	if err != nil {
		return err
	}
	result.Verb = output.Remove
	result.Details["removed"] = true
	return termVerb(a, stdout, stderr, result, warnings)
}
