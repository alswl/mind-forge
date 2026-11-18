package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/lifecycle"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("lifecycle status", "lifecycle", newLifecycleStatusCommand, func(e commandExecution) error {
		return executeLifecycleStatus(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newLifecycleStatusCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "status"}
	return configureCommand(command, item, handler)
}

func executeLifecycleStatus(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {

	return executeLifecycleStatusDetails(command, application, args, stdout, stderr)
}

func executeLifecycleStatusDetails(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
	engine := lifecycle.New()
	root := application.RepoRoot
	var data map[string]any

	var err error
	data, err = engine.Status(root)
	if err != nil {
		return err
	}

	if application.Quiet {
		return nil
	}
	return output.RenderSuccess(stdout, application.Format, data)
}
