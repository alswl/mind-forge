package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/lifecycle"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("lifecycle resume", "lifecycle", newLifecycleResumeCommand, func(e commandExecution) error {
		return executeLifecycleResume(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newLifecycleResumeCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "resume"}
	return configureCommand(command, item, handler)
}

func executeLifecycleResume(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {

	return executeLifecycleResumeDetails(command, application, args, stdout, stderr)
}

func executeLifecycleResumeDetails(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
	engine := lifecycle.New()
	root := application.RepoRoot
	var data map[string]any

	var plan *lifecycle.Plan
	if name := optionalFlag(command, "plan"); name != nil {
		loaded, err := lifecycle.LoadPlan(application.CWD, *name)
		if err != nil {
			return err
		}
		plan = &loaded
	}
	var err error
	data, err = engine.Recover(command.Context(), root, false, plan)
	if err != nil {
		return err
	}

	if application.Quiet {
		return nil
	}
	return output.RenderSuccess(stdout, application.Format, data)
}
