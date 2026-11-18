package main

import (
	"io"

	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("project index", "project", newProjectIndexCommand, func(e commandExecution) error {
		return executeProjectIndex(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newProjectIndexCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "index"}
	return configureCommand(command, item, handler)
}

func executeProjectIndex(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {

	return projectIndexCommand(command, application, stdout, stderr)
}
