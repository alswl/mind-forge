package main

import (
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("publish", "", newPublishCommand, func(e commandExecution) error {
		return e.Command.Help()
	})
}

func newPublishCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "publish"}
	return configureCommand(command, item, handler)
}
