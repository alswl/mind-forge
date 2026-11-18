package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/lifecycle"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("lifecycle rollback", "lifecycle", newLifecycleRollbackCommand, func(e commandExecution) error {
		return executeLifecycleRollback(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newLifecycleRollbackCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "rollback"}
	return configureCommand(command, item, handler)
}

func executeLifecycleRollback(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {

	return executeLifecycleRollbackDetails(command, application, args, stdout, stderr)
}

func executeLifecycleRollbackDetails(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
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
	data, err = engine.Recover(command.Context(), root, true, plan)
	if err != nil {
		return err
	}

	if application.Quiet {
		return nil
	}
	return output.RenderSuccess(stdout, application.Format, data)
}
