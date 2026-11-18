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
	registerCommand("output show", "output", newOutputShowCommand, func(e commandExecution) error {
		return executeOutputShow(e.Command, e.Application, e.Args, e.Stdout)
	})
}

func newOutputShowCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "show"}
	return configureCommand(command, item, handler)
}

func executeOutputShow(command *cobra.Command, application *app.Context, args []string, stdout io.Writer) error {

	var selector *string

	selector = &args[0]

	project, err := content.ProjectFor(application.RepoRoot, application.CWD, optionalFlag(command, "project"), selector)
	if err != nil {
		return err
	}
	root := application.RepoRoot
	var data map[string]any

	var item model.Output
	item, err = content.ResolveOutput(root, project, args[0])
	if err == nil {
		data, err = jsonObject(item)
	}

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
