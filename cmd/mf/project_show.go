package main

import (
	"io"

	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("project show", "project", newProjectShowCommand, func(e commandExecution) error {
		return executeProjectShow(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newProjectShowCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "show"}
	return configureCommand(command, item, handler)
}

func executeProjectShow(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {

	return showProject(application, args[0], stdout)
}
