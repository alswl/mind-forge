package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("additional list", "additional", newAdditionalListCommand, func(e commandExecution) error {
		return executeAdditionalList(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newAdditionalListCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "list"}
	return configureCommand(command, item, handler)
}

func executeAdditionalList(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
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

		if selector != nil {
			mind, err := content.ResolveMind(application.RepoRoot, project, *selector)
			if err != nil {
				return err
			}
			filtered := make([]content.Additional, 0, len(items))
			for _, item := range items {
				if item.Mind == mind.Name {
					filtered = append(filtered, item)
				}
			}
			items = filtered
		}
		data = map[string]any{"kind": "additional", "additional": items}

	}

	if application.Quiet {
		return nil
	}
	return output.RenderSuccess(stdout, application.Format, data)
}
