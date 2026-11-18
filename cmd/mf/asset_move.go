package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/asset"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("asset move", "asset", newAssetMoveCommand, func(e commandExecution) error {
		return executeAssetMove(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newAssetMoveCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "move"}
	return configureCommand(command, item, handler)
}

func executeAssetMove(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
	project, err := app.ResolveProject(application.RepoRoot, optionalFlag(command, "project"), application.CWD, false)
	if err != nil {
		return err
	}
	view := *application
	view.RepoRoot = project
	service := asset.Service{}
	dryRun := boolFlag(command, "dry-run")
	result := output.VerbResult{Kind: "asset", DryRun: dryRun, Details: map[string]any{}}
	{

		if err := content.ValidateEntityPath(project, args[0]); err != nil {
			return err
		}
		target, err := app.ResolveProject(application.RepoRoot, optionalFlag(command, "to-project"), application.CWD, false)
		if err != nil {
			return err
		}
		report, err := service.Move(project, target, args[0], dryRun)
		if err != nil {
			return err
		}
		result.Verb, result.Identity, result.OldIdentity = output.Move, report.NewPath, &report.OldPath
		if !dryRun {
			result.Path = &report.NewPath
		}
		result.Details = map[string]any{"old_path": report.OldPath, "new_path": report.NewPath, "from_scope": project, "to_scope": target}
		view.RepoRoot = application.RepoRoot
	}

	return renderProjectVerb(stdout, &view, result)
}
