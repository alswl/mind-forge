package main

import (
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("lifecycle", "", newLifecycleCommand, func(e commandExecution) error {
		return e.Command.Help()
	})
}

func newLifecycleCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "lifecycle"}
	return configureCommand(command, item, handler)
}
