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
	registerCommand("lifecycle output-remove", "lifecycle", newLifecycleOutputRemoveCommand, func(e commandExecution) error {
		return executeLifecycleOutputRemove(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newLifecycleOutputRemoveCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "output-remove"}
	return configureCommand(command, item, handler)
}

func executeLifecycleOutputRemove(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {

	var selector *string
	if len(args) > 0 {
		selector = &args[0]
	}
	project, err := content.ProjectFor(application.RepoRoot, application.CWD, optionalFlag(command, "project"), selector)
	if err != nil {
		return err
	}
	operation := content.LifecycleOperation{Context: command.Context(), Kind: "output-remove"}
	if selector != nil {
		operation.Identity = *selector
	}
	if len(args) > 1 {
		operation.Name = args[1]
	}
	if target := optionalFlag(command, "to"); target != nil {
		operation.Layout = *target
	}
	if target := optionalFlag(command, "to-project"); target != nil {
		operation.Project = *target
	}
	if target := optionalFlag(command, "to-mind"); target != nil {
		operation.Mind = *target
	}
	plan, err := content.PlanLifecycle(application.RepoRoot, project, operation)
	if err != nil {
		return err
	}
	data, err := executeLifecyclePlan(command, application, lifecycle.New(), plan, stderr)
	if err != nil {
		return err
	}
	if application.Quiet {
		return nil
	}
	return output.RenderSuccess(stdout, application.Format, data)
}
