package main

import (
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("output", "", newOutputCommand, func(e commandExecution) error {
		return e.Command.Help()
	})
}

func newOutputCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "output"}
	return configureCommand(command, item, handler)
}
