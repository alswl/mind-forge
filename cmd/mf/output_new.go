package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("output new", "output", newOutputNewCommand, func(e commandExecution) error {
		return executeOutputNew(e.Command, e.Application, e.Args, e.Stdout)
	})
}

func newOutputNewCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "new"}
	return configureCommand(command, item, handler)
}

func executeOutputNew(command *cobra.Command, application *app.Context, args []string, stdout io.Writer) error {

	var selector *string

	selector = &args[0]

	project, err := content.ProjectFor(application.RepoRoot, application.CWD, optionalFlag(command, "project"), selector)
	if err != nil {
		return err
	}
	root := application.RepoRoot
	var data map[string]any

	data, err = content.CreateOutput(root, project, args[0], args[1], content.OutputCreateOptions{Title: optionalFlag(command, "title"), File: boolFlag(command, "file"), DryRun: boolFlag(command, "dry-run")})

	if err != nil {
		return err
	}
	if application.Quiet {
		return nil
	}
	if application.Format == "json" {
		return output.RenderSuccess(stdout, "json", data)
	}
	return renderMindText(data, application, stdout)
}
