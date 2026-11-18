package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/term"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("term rename", "term", newTermRenameCommand, func(e commandExecution) error {
		return executeTermRename(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newTermRenameCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "rename"}
	return configureCommand(command, item, handler)
}

func executeTermRename(c *cobra.Command, a *app.Context, args []string, stdout, stderr io.Writer) error {

	return executeTermRenameDetails(c, a, args, stdout, stderr)
}

func executeTermRenameDetails(c *cobra.Command, a *app.Context, args []string, stdout, stderr io.Writer) error {
	dry, force := boolFlag(c, "dry-run"), boolFlag(c, "force")
	result := output.VerbResult{Kind: "term", Identity: args[0], DryRun: dry, Details: map[string]any{}}

	result.Verb = output.Rename
	result.Identity = args[1]
	result.OldIdentity = &args[0]
	result.Details["keep_alias"] = boolFlag(c, "keep-alias")
	if dry {
		return termVerb(a, stdout, stderr, result, nil)
	}
	_, _, warnings, err := termFallback(c, a, args[0], func(s term.Scope) (bool, error) {
		return true, s.Rename(args[0], args[1], boolFlag(c, "keep-alias"), force)
	})
	if err != nil {
		return err
	}
	return termVerb(a, stdout, stderr, result, warnings)
}
