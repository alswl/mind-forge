package main

import (
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("publish target", "publish", newPublishTargetCommand, func(e commandExecution) error {
		return e.Command.Help()
	})
}

func newPublishTargetCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "target"}
	return configureCommand(command, item, handler)
}
