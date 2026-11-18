package main

import (
	"io"

	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("project update", "project", newProjectUpdateCommand, func(e commandExecution) error {
		return executeProjectUpdate(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newProjectUpdateCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "update"}
	return configureCommand(command, item, handler)
}

func executeProjectUpdate(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
	root := application.RepoRoot
	dryRun := boolFlag(command, "dry-run")

	result, err := content.UpdateProject(root, args[0], optionalFlag(command, "description"), boolFlag(command, "clear-description"), dryRun)
	if err != nil {
		return err
	}
	return renderProjectVerb(stdout, application, result)
}
