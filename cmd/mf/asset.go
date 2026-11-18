package main

import (
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("asset", "", newAssetCommand, func(e commandExecution) error {
		return e.Command.Help()
	})
}

func newAssetCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "asset"}
	return configureCommand(command, item, handler)
}
