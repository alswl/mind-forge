package main

import (
	"io"

	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("config terminal", "config", newConfigTerminalCommand, func(e commandExecution) error {
		return executeConfigTerminal(e.Command, e.Application, e.Stdout)
	})
}

func newConfigTerminalCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "terminal"}
	return configureCommand(command, item, handler)
}

func executeConfigTerminal(command *cobra.Command, application *app.Context, stdout io.Writer) error {

	return terminalCommand(application, stdout)
}
