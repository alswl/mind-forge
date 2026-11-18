package main

import (
	"io"

	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("project lint", "project", newProjectLintCommand, func(e commandExecution) error {
		return executeProjectLint(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newProjectLintCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "lint"}
	return configureCommand(command, item, handler)
}

func executeProjectLint(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {

	return projectLintCommand(command, application, stdout)
}
