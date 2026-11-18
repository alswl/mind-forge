package cli

import (
	"fmt"
	"io"

	"github.com/alswl/mind-forge/internal/app"
	"github.com/alswl/mind-forge/internal/content"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/spf13/cobra"
)

func promptCommand(command *cobra.Command, application *app.Context, args []string, stdout io.Writer) error {
	project, err := app.ResolveProject(application.RepoRoot, optionalFlag(command, "project"), application.CWD, false)
	if err != nil {
		return err
	}
	items, err := content.PromptProjection(project)
	if err != nil {
		return err
	}
	if command.Name() == "show" {
		for _, item := range items {
			if item.Path == args[0] {
				if application.Quiet {
					return nil
				}
				if application.Format == "json" {
					return output.RenderSuccess(stdout, "json", item)
				}
				mode := ""
				if item.Mode != nil {
					mode = *item.Mode
				}
				return output.RenderRaw(stdout, "text", fmt.Sprintf("path  %s\narticle  %s\nmode  %s\nupdated  %s", item.Path, item.Article, mode, item.UpdatedAt))
			}
		}
		return output.Usage("prompt '"+args[0]+"' not found", "use `mf prompt list` to see available prompts")
	}
	if application.Quiet {
		return nil
	}
	if application.Format == "json" {
		return output.RenderSuccess(stdout, "json", map[string]any{"prompts": items})
	}
	view := output.List{Headers: []string{"PATH", "ARTICLE", "MODE", "UPDATED"}, Plural: "prompts"}
	for _, item := range items {
		mode := ""
		if item.Mode != nil {
			mode = *item.Mode
		}
		cells := output.TextCells(item.Path, item.Article, mode, item.UpdatedAt)
		cells[0].Path, cells[1].Path = true, true
		view.Rows = append(view.Rows, cells)
	}
	options := output.ListOptionsFromProfile(commandProfile(stdout, application), false, false)
	return output.RenderRaw(stdout, "text", output.RenderList(view, options))
}
