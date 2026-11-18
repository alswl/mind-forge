package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("article block rename", "article block", newArticleBlockRenameCommand, func(e commandExecution) error {
		return executeArticleBlockRename(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newArticleBlockRenameCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "rename"}
	return configureCommand(command, item, handler)
}

func executeArticleBlockRename(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
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

	old := article + "/" + args[1]
	result.Verb, result.Identity, result.OldIdentity, result.Details = output.Rename, article+"/"+args[2], &old, map[string]any{}
	if _, err := content.CheckedPath(project, article); err != nil {
		return err
	}
	if !dryRun {
		report, err := content.RenameBlock(project, article, args[1], args[2], force)
		if err != nil {
			return err
		}
		oldPath, newPath := article+"/"+report.OldFilename, article+"/"+report.NewFilename
		result.Identity, result.OldIdentity, result.Path = newPath, &oldPath, &newPath
		result.Details = map[string]any{"old_filename": report.OldFilename, "new_filename": report.NewFilename, "article_path": report.ArticlePath}
	}

	view := *application
	view.RepoRoot = project
	return renderProjectVerb(stdout, &view, result)
}
