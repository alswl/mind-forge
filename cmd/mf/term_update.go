package main

import (
	"io"

	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("term update", "term", newTermUpdateCommand, func(e commandExecution) error {
		return executeTermUpdate(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newTermUpdateCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "update"}
	return configureCommand(command, item, handler)
}

func executeTermUpdate(c *cobra.Command, a *app.Context, args []string, stdout, stderr io.Writer) error {

	return termUpdateCommand(c, a, args[0], stdout, stderr)
}
