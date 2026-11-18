package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("article block new", "article block", newArticleBlockNewCommand, func(e commandExecution) error {
		return executeArticleBlockNew(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newArticleBlockNewCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "new"}
	return configureCommand(command, item, handler)
}

func executeArticleBlockNew(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
	project, err := app.ResolveProject(application.RepoRoot, optionalFlag(command, "project"), application.CWD, false)
	if err != nil {
		return err
	}
	article, err := content.ResolveArticleSelector(project, args[0])
	if err != nil {
		return err
	}
	dryRun, _ := boolFlag(command, "dry-run"), boolFlag(command, "force")
	result := output.VerbResult{Kind: "block", DryRun: dryRun}

	start, _ := command.Flags().GetUint64("start")
	options := content.BlockEditOptions{After: optionalFlag(command, "after"), Start: start, DryRun: dryRun}
	var report content.BlockEditReport

	report, err = content.NewBlock(project, article, args[1], options)

	if err != nil {
		return err
	}
	result.Verb, result.Identity, result.Path, result.OldIdentity = output.Update, article, report.NewPath, report.OldPath
	if report.NewPath != nil {
		result.Identity = *report.NewPath
	} else {
		if report.OldPath != nil {
			result.Identity = *report.OldPath
		}
	}
	result.Details = map[string]any{"article_path": report.ArticlePath, "old_path": report.OldPath, "new_path": report.NewPath, "order": report.Order}

	view := *application
	view.RepoRoot = project
	return renderProjectVerb(stdout, &view, result)
}
