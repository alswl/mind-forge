package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/pkg/common/model"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("mind list", "mind", newMindListCommand, func(e commandExecution) error {
		return executeMindList(e.Command, e.Application, e.Args, e.Stdout)
	})
}

func newMindListCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "list"}
	return configureCommand(command, item, handler)
}

func executeMindList(command *cobra.Command, application *app.Context, args []string, stdout io.Writer) error {

	var selector *string

	project, err := content.ProjectFor(application.RepoRoot, application.CWD, optionalFlag(command, "project"), selector)
	if err != nil {
		return err
	}
	root := application.RepoRoot
	var data map[string]any

	var minds []model.Mind
	minds, err = content.Discover(root, project)
	data = map[string]any{"minds": minds}

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
