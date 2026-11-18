package main

import (
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("render template", "render", newRenderTemplateCommand, func(e commandExecution) error {
		return e.Command.Help()
	})
}

func newRenderTemplateCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "template"}
	return configureCommand(command, item, handler)
}
