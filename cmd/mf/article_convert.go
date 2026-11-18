package main

import (
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("article convert", "article", newArticleConvertCommand, func(e commandExecution) error {
		return articleConvertCommand(e.Command, e.Application, e.Stdout, e.Stderr)
	})
}

func newArticleConvertCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "convert"}
	return configureCommand(command, item, handler)
}
