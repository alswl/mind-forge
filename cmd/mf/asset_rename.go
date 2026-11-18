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
	registerCommand("asset rename", "asset", newAssetRenameCommand, func(e commandExecution) error {
		return executeAssetRename(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newAssetRenameCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "rename"}
	return configureCommand(command, item, handler)
}

func executeAssetRename(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
	project, err := app.ResolveProject(application.RepoRoot, optionalFlag(command, "project"), application.CWD, false)
	if err != nil {
		return err
	}
	view := *application
	view.RepoRoot = project
	service := asset.Service{}
	dryRun := boolFlag(command, "dry-run")
	result := output.VerbResult{Kind: "asset", DryRun: dryRun, Details: map[string]any{}}

	for _, path := range args {
		if err := content.ValidateEntityPath(project, path); err != nil {
			return err
		}
	}
	result.Verb, result.Identity, result.OldIdentity = output.Rename, args[1], &args[0]
	if !dryRun {
		if err := service.Rename(project, args[0], args[1], boolFlag(command, "force")); err != nil {
			return err
		}
		result.Path = &args[1]
	}

	return renderProjectVerb(stdout, &view, result)
}
