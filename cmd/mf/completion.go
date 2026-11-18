package main

import (
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("completion", "", newCompletionCommand, func(e commandExecution) error {
		return completionCommand(e.Command, e.Args[0], e.Stdout)
	})
}

func newCompletionCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "completion"}
	return configureCommand(command, item, handler)
}
