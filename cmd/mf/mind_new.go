package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("mind new", "mind", newMindNewCommand, func(e commandExecution) error {
		return executeMindNew(e.Command, e.Application, e.Args, e.Stdout)
	})
}

func newMindNewCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "new"}
	return configureCommand(command, item, handler)
}

func executeMindNew(command *cobra.Command, application *app.Context, args []string, stdout io.Writer) error {

	var selector *string

	project, err := content.ProjectFor(application.RepoRoot, application.CWD, optionalFlag(command, "project"), selector)
	if err != nil {
		return err
	}
	root := application.RepoRoot
	var data map[string]any

	data, err = content.CreateMind(root, project, args[0], content.MindCreateOptions{Title: optionalFlag(command, "title"), DefaultOutput: optionalFlag(command, "default-output"), DryRun: boolFlag(command, "dry-run")})

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
