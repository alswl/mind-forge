package main

import (
	"io"

	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("project new", "project", newProjectNewCommand, func(e commandExecution) error {
		return executeProjectNew(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newProjectNewCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "new"}
	return configureCommand(command, item, handler)
}

func executeProjectNew(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
	root := application.RepoRoot
	dryRun := boolFlag(command, "dry-run")
	force := boolFlag(command, "force")

	result, err := content.CreateProject(root, application.CWD, args[0], content.ProjectCreateOptions{Force: force, DryRun: dryRun, Layout: optionalFlag(command, "layout")})
	if err != nil {
		return err
	}
	return renderProjectVerb(stdout, application, result)
}
