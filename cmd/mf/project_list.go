package main

import (
	"io"

	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("project list", "project", newProjectListCommand, func(e commandExecution) error {
		return executeProjectList(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newProjectListCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "list"}
	return configureCommand(command, item, handler)
}

func executeProjectList(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {

	return listProjects(command, application, stdout, stderr)
}
