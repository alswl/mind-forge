package main

import (
	"io"
	"time"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("article rename", "article", newArticleRenameCommand, func(e commandExecution) error {
		return executeArticleRename(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newArticleRenameCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "rename"}
	return configureCommand(command, item, handler)
}

func executeArticleRename(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
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
	if err := content.ValidateArticleArgument(project, args[1]); err != nil {
		return err
	}
	if dryRun {
		old := args[0]
		return renderProjectVerb(stdout, &view, output.VerbResult{Verb: output.Rename, Kind: "article", Identity: args[1], OldIdentity: &old, DryRun: true, Details: map[string]any{}})
	}
	mutation, err = content.PlanArticleRename(root, project, args[0], args[1], force, time.Time{})

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
