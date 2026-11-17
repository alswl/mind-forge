package cli

import (
	"io"

	"github.com/alswl/mind-forge/internal/app"
	"github.com/alswl/mind-forge/internal/content"
	"github.com/alswl/mind-forge/internal/lifecycle"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/spf13/cobra"
)

func additionalCommand(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
	var selector *string
	if len(args) > 0 {
		selector = &args[0]
	}
	project, err := content.ProjectFor(application.RepoRoot, application.CWD, optionalFlag(command, "project"), selector)
	if err != nil {
		return err
	}
	var data map[string]any
	if command.Name() == "list" || command.Name() == "show" {
		items, err := content.DiscoverAdditional(application.RepoRoot, project)
		if err != nil {
			return err
		}
		if command.Name() == "show" {
			item, err := content.SelectAdditional(items, args[0])
			if err != nil {
				return err
			}
			data, err = jsonObject(item)
			if err != nil {
				return err
			}
		} else {
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
	} else {
		var destination *string
		if command.Name() == "rename" {
			destination = &args[1]
		}
		plan, err := content.PlanAdditional(application.RepoRoot, project, args[0], destination)
		if err != nil {
			return err
		}
		data, err = executeLifecyclePlan(command, application, lifecycle.Engine{}, plan, stderr)
		if err != nil {
			return err
		}
	}
	if application.Quiet {
		return nil
	}
	return output.RenderSuccess(stdout, application.Format, data)
}
