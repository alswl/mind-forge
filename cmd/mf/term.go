package main

import (
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("term", "", newTermCommand, func(e commandExecution) error {
		return e.Command.Help()
	})
}

func newTermCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "term"}
	return configureCommand(command, item, handler)
}
