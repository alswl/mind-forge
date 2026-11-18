package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("article update", "article", newArticleUpdateCommand, func(e commandExecution) error {
		return executeArticleUpdate(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newArticleUpdateCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "update"}
	return configureCommand(command, item, handler)
}

func executeArticleUpdate(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
	project, err := app.ResolveProject(application.RepoRoot, optionalFlag(command, "project"), application.CWD, false)
	if err != nil {
		return err
	}
	dryRun := boolFlag(command, "dry-run")
	view := *application
	view.RepoRoot = project

	report, err := content.UpdateArticle(project, args[0], content.ArticleUpdateOptions{Title: optionalFlag(command, "title"), Status: optionalFlag(command, "status"), DryRun: dryRun})
	if err != nil {
		return err
	}
	path := report.Article.Path
	result := output.VerbResult{Verb: output.Update, Kind: "article", Identity: path, Path: &path, DryRun: dryRun, Details: map[string]any{"changes": report.Changes, "article": report.Article}}
	return renderProjectVerb(stdout, &view, result)
}
