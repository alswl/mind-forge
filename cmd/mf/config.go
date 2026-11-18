package main

import (
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("config", "", newConfigCommand, func(e commandExecution) error {
		return e.Command.Help()
	})
}

func newConfigCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "config"}
	return configureCommand(command, item, handler)
}
