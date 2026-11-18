package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/alswl/mind-forge/pkg/services/source"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("source rename", "source", newSourceRenameCommand, func(e commandExecution) error {
		return executeSourceRename(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newSourceRenameCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "rename"}
	return configureCommand(command, item, handler)
}

func executeSourceRename(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {

	project, err := app.ResolveProject(application.RepoRoot, optionalFlag(command, "project"), application.CWD, false)
	if err != nil {
		return err
	}
	view := *application
	view.RepoRoot = project
	service := source.Service{}
	dryRun := boolFlag(command, "dry-run")
	result := output.VerbResult{Kind: "source", DryRun: dryRun, Details: map[string]any{}}
	{

		if err := content.ValidateEntityPath(project, args[0]); err != nil {
			return err
		}

		items, err := source.List(project, nil, nil)
		if err != nil {
			return err
		}
		name, err := source.ResolveSelector(items, args[0])
		if err != nil {
			return err
		}
		{

			report, err := service.Rename(project, application.CWD, name, args[1], boolFlag(command, "force"), dryRun)
			if err != nil {
				return err
			}
			result.Verb, result.Identity, result.OldIdentity, result.Path = output.Rename, report.After.Name, &report.Before.Name, report.After.Path

		}
	}

	return renderProjectVerb(stdout, &view, result)
}
