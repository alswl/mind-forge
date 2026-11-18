package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("prompt", "", newPromptCommand, func(e commandExecution) error {
		return executePrompt(e.Command, e.Application, e.Args, e.Stdout)
	})
}

func newPromptCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "prompt"}
	return configureCommand(command, item, handler)
}

func executePrompt(command *cobra.Command, application *app.Context, args []string, stdout io.Writer) error {
	project, err := app.ResolveProject(application.RepoRoot, optionalFlag(command, "project"), application.CWD, false)
	if err != nil {
		return err
	}
	items, err := content.PromptProjection(project)
	if err != nil {
		return err
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
