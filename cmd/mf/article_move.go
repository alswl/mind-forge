package main

import (
	"io"

	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("article move", "article", newArticleMoveCommand, func(e commandExecution) error {
		return executeArticleMove(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newArticleMoveCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "move"}
	return configureCommand(command, item, handler)
}

func executeArticleMove(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
	root := application.RepoRoot
	project, err := app.ResolveProject(root, optionalFlag(command, "project"), application.CWD, false)
	if err != nil {
		return err
	}
	dryRun, _ := boolFlag(command, "dry-run"), boolFlag(command, "force")
	view := *application
	view.RepoRoot = project
	var mutation content.ContentMutation

	target, resolveErr := app.ResolveProject(root, optionalFlag(command, "to-project"), application.CWD, false)
	if resolveErr != nil {
		return resolveErr
	}
	mutation, err = content.PlanArticleMove(root, project, target, args[0], dryRun)
	view.RepoRoot = root

	if err != nil {
		return err
	}
	if !dryRun {
		if err := content.ApplyMutation(command.Context(), root, mutation); err != nil {
			return err
		}
	}
	return renderProjectVerb(stdout, &view, mutation.Result)
}
