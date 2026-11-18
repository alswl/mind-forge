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
	registerCommand("source move", "source", newSourceMoveCommand, func(e commandExecution) error {
		return executeSourceMove(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newSourceMoveCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "move"}
	return configureCommand(command, item, handler)
}

func executeSourceMove(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {

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

			target, err := app.ResolveProject(application.RepoRoot, optionalFlag(command, "to-project"), application.CWD, false)
			if err != nil {
				return err
			}
			report, err := service.Move(project, target, name, dryRun)
			if err != nil {
				return err
			}
			result.Verb, result.Identity, result.OldIdentity, result.Path = output.Move, report.Name, &report.OldPath, &report.NewPath
			result.Details = map[string]any{"name": report.Name, "old_path": report.OldPath, "new_path": report.NewPath, "dry_run": report.DryRun}
			view.RepoRoot = application.RepoRoot

		}
	}

	return renderProjectVerb(stdout, &view, result)
}
