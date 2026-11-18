package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("mind index", "mind", newMindIndexCommand, func(e commandExecution) error {
		return executeMindIndex(e.Command, e.Application, e.Args, e.Stdout)
	})
}

func newMindIndexCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "index"}
	return configureCommand(command, item, handler)
}

func executeMindIndex(command *cobra.Command, application *app.Context, args []string, stdout io.Writer) error {

	var selector *string

	project, err := content.ProjectFor(application.RepoRoot, application.CWD, optionalFlag(command, "project"), selector)
	if err != nil {
		return err
	}
	root := application.RepoRoot
	var data map[string]any

	if boolFlag(command, "check") {
		data, err = content.InspectContent(root, project)
		if err != nil {
			return err
		}
		if !application.Quiet {
			if err := output.RenderSuccess(stdout, application.Format, data); err != nil {
				return err
			}
		}
		if data["consistent"] != true {
			return &commandExit{1}
		}
		return nil
	}
	data, err = content.ReconcileContent(root, project, boolFlag(command, "dry-run"))

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
