package main

import (
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("additional", "", newAdditionalCommand, func(e commandExecution) error {
		return e.Command.Help()
	})
}

func newAdditionalCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "additional"}
	return configureCommand(command, item, handler)
}
