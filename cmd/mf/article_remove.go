package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("article remove", "article", newArticleRemoveCommand, func(e commandExecution) error {
		return executeArticleRemove(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newArticleRemoveCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "remove"}
	return configureCommand(command, item, handler)
}

func executeArticleRemove(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
	root := application.RepoRoot
	project, err := app.ResolveProject(root, optionalFlag(command, "project"), application.CWD, false)
	if err != nil {
		return err
	}
	dryRun, force := boolFlag(command, "dry-run"), boolFlag(command, "force")
	view := *application
	view.RepoRoot = project
	var mutation content.ContentMutation

	if err := content.ValidateArticleArgument(project, args[0]); err != nil {
		return err
	}
	if err := output.RequireConfirmation(output.ConfirmArgs{VerbLabel: "removal", Kind: "article", Identity: args[0], Yes: boolFlag(command, "yes"), Force: force}, stderr, nil); err != nil {
		return err
	}
	mutation, err = content.PlanArticleRemove(root, project, args[0], dryRun)

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
