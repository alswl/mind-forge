package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/alswl/mind-forge/pkg/services/lifecycle"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("additional remove", "additional", newAdditionalRemoveCommand, func(e commandExecution) error {
		return executeAdditionalRemove(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newAdditionalRemoveCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "remove"}
	return configureCommand(command, item, handler)
}

func executeAdditionalRemove(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
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

		var destination *string

		plan, err := content.PlanAdditional(application.RepoRoot, project, args[0], destination)
		if err != nil {
			return err
		}
		data, err = executeLifecyclePlan(command, application, lifecycle.New(), plan, stderr)
		if err != nil {
			return err
		}
	}

	if application.Quiet {
		return nil
	}
	return output.RenderSuccess(stdout, application.Format, data)
}
