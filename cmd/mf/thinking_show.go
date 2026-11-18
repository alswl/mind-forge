package main

import (
	"fmt"
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("thinking show", "thinking", newThinkingShowCommand, func(e commandExecution) error {
		return executeThinkingShow(e.Command, e.Application, e.Args, e.Stdout)
	})
}

func newThinkingShowCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "show"}
	return configureCommand(command, item, handler)
}

func executeThinkingShow(command *cobra.Command, application *app.Context, args []string, stdout io.Writer) error {
	project, err := app.ResolveProject(application.RepoRoot, optionalFlag(command, "project"), application.CWD, false)
	if err != nil {
		return err
	}
	items, err := content.ThinkingProjection(project)
	if err != nil {
		return err
	}

	for _, item := range items {
		if item.Path == args[0] {
			if application.Quiet {
				return nil
			}
			if application.Format == "json" {
				return output.RenderSuccess(stdout, "json", item)
			}
			return output.RenderRaw(stdout, "text", fmt.Sprintf("path  %s\narticle  %s\nupdated  %s", item.Path, item.Article, item.UpdatedAt))
		}
	}
	return output.Usage("thinking '"+args[0]+"' not found", "use `mf thinking list` to see available entries")
}
