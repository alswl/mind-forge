package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("additional show", "additional", newAdditionalShowCommand, func(e commandExecution) error {
		return executeAdditionalShow(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newAdditionalShowCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "show"}
	return configureCommand(command, item, handler)
}

func executeAdditionalShow(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
	var selector *string
	if len(args) > 0 {
		selector = &args[0]
	}
	project, err := content.ProjectFor(application.RepoRoot, application.CWD, optionalFlag(command, "project"), selector)
	if err != nil {
		return err
	}
	var data map[string]any
	{

		items, err := content.DiscoverAdditional(application.RepoRoot, project)
		if err != nil {
			return err
		}
		{

			item, err := content.SelectAdditional(items, args[0])
			if err != nil {
				return err
			}
			data, err = jsonObject(item)
			if err != nil {
				return err
			}
		}
	}

	if application.Quiet {
		return nil
	}
	return output.RenderSuccess(stdout, application.Format, data)
}
