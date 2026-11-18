package main

import (
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("project", "", newProjectCommand, func(e commandExecution) error {
		return e.Command.Help()
	})
}

func newProjectCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "project"}
	return configureCommand(command, item, handler)
}
