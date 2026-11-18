package main

import (
	"io"

	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("term lint", "term", newTermLintCommand, func(e commandExecution) error {
		return executeTermLint(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newTermLintCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "lint"}
	return configureCommand(command, item, handler)
}

func executeTermLint(c *cobra.Command, a *app.Context, args []string, stdout, stderr io.Writer) error {

	return termLintCommand(c, a, args, stdout, stderr)
}
