package main

import (
	"io"
	"time"

	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("article index", "article", newArticleIndexCommand, func(e commandExecution) error {
		return executeArticleIndex(e.Command, e.Application, e.Stdout, e.Stderr)
	})
}

func newArticleIndexCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "index"}
	return configureCommand(command, item, handler)
}

func executeArticleIndex(command *cobra.Command, application *app.Context, stdout, stderr io.Writer) error {
	project, err := app.ResolveProject(application.RepoRoot, optionalFlag(command, "project"), application.CWD, false)
	if err != nil {
		return err
	}
	dryRun := boolFlag(command, "dry-run")
	view := *application
	view.RepoRoot = project
	{

		report, warnings, err := content.IndexArticles(project, dryRun, time.Time{})
		if err != nil {
			return err
		}
		if err := articleWarnings(stderr, warnings); err != nil {
			return err
		}
		return renderProjectVerb(stdout, &view, report)
	}
}
