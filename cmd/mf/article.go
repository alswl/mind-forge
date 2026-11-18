package main

import (
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("article", "", newArticleCommand, func(e commandExecution) error {
		return e.Command.Help()
	})
}

func newArticleCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "article"}
	return configureCommand(command, item, handler)
}
