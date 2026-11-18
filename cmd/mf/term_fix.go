package main

import (
	"io"

	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("term fix", "term", newTermFixCommand, func(e commandExecution) error {
		return executeTermFix(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newTermFixCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "fix"}
	return configureCommand(command, item, handler)
}

func executeTermFix(c *cobra.Command, a *app.Context, args []string, stdout, stderr io.Writer) error {

	return termLintCommand(c, a, args, stdout, stderr)
}
