package main

import (
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("term correction", "term", newTermCorrectionCommand, func(e commandExecution) error {
		return e.Command.Help()
	})
}

func newTermCorrectionCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "correction"}
	return configureCommand(command, item, handler)
}
