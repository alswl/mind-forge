package main

import (
	"io"

	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("term list", "term", newTermListCommand, func(e commandExecution) error {
		return executeTermList(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newTermListCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "list"}
	return configureCommand(command, item, handler)
}

func executeTermList(c *cobra.Command, a *app.Context, args []string, stdout, stderr io.Writer) error {

	return termListCommand(c, a, stdout, stderr)
}
