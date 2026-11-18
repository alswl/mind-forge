package main

import (
	"io"

	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("term show", "term", newTermShowCommand, func(e commandExecution) error {
		return executeTermShow(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newTermShowCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "show"}
	return configureCommand(command, item, handler)
}

func executeTermShow(c *cobra.Command, a *app.Context, args []string, stdout, stderr io.Writer) error {

	return termShowCommand(c, a, args[0], stdout, stderr)
}
