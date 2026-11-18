package main

import (
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("mind", "", newMindCommand, func(e commandExecution) error {
		return e.Command.Help()
	})
}

func newMindCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "mind"}
	return configureCommand(command, item, handler)
}
