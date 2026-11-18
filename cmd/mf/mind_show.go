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
	registerCommand("mind show", "mind", newMindShowCommand, func(e commandExecution) error {
		return executeMindShow(e.Command, e.Application, e.Args, e.Stdout)
	})
}

func newMindShowCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "show"}
	return configureCommand(command, item, handler)
}

func executeMindShow(command *cobra.Command, application *app.Context, args []string, stdout io.Writer) error {

	var selector *string

	selector = &args[0]

	project, err := content.ProjectFor(application.RepoRoot, application.CWD, optionalFlag(command, "project"), selector)
	if err != nil {
		return err
	}
	root := application.RepoRoot
	var data map[string]any

	var mind model.Mind
	mind, err = content.ResolveMind(root, project, args[0])
	if err == nil {
		data, err = jsonObject(mind)
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
