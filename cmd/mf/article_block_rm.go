package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("article block rm", "article block", newArticleBlockRmCommand, func(e commandExecution) error {
		return executeArticleBlockRm(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newArticleBlockRmCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "rm"}
	return configureCommand(command, item, handler)
}

func executeArticleBlockRm(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
	project, err := app.ResolveProject(application.RepoRoot, optionalFlag(command, "project"), application.CWD, false)
	if err != nil {
		return err
	}
	article, err := content.ResolveArticleSelector(project, args[0])
	if err != nil {
		return err
	}
	dryRun, force := boolFlag(command, "dry-run"), boolFlag(command, "force")
	result := output.VerbResult{Kind: "block", DryRun: dryRun}
	{

		if _, err := content.CheckedPath(project, article); err != nil {
			return err
		}
		if !dryRun {
			if err := output.RequireConfirmation(output.ConfirmArgs{VerbLabel: "removal", Kind: "block", Identity: article + "/" + args[1], Yes: boolFlag(command, "yes"), Force: force}, stderr, nil); err != nil {
				return err
			}
		}
		report, err := content.RemoveBlock(project, article, args[1], dryRun)
		if err != nil {
			return err
		}
		path := report.ArticlePath + "/" + report.RemovedFilename
		result.Verb, result.Identity, result.Path = output.Remove, path, &path
		result.Details = map[string]any{"article_path": report.ArticlePath, "removed_filename": report.RemovedFilename, "remaining_blocks": report.RemainingBlocks}
	}

	view := *application
	view.RepoRoot = project
	return renderProjectVerb(stdout, &view, result)
}
