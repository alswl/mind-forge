package main

import (
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("source", "", newSourceCommand, func(e commandExecution) error {
		return e.Command.Help()
	})
}

func newSourceCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "source"}
	return configureCommand(command, item, handler)
}
