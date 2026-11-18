package main

import (
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("article block", "article", newArticleBlockCommand, func(e commandExecution) error {
		return e.Command.Help()
	})
}

func newArticleBlockCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "block"}
	return configureCommand(command, item, handler)
}
