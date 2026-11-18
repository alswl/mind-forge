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
	registerCommand("asset remove", "asset", newAssetRemoveCommand, func(e commandExecution) error {
		return executeAssetRemove(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newAssetRemoveCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "remove"}
	return configureCommand(command, item, handler)
}

func executeAssetRemove(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
	project, err := app.ResolveProject(application.RepoRoot, optionalFlag(command, "project"), application.CWD, false)
	if err != nil {
		return err
	}
	view := *application
	view.RepoRoot = project
	service := asset.Service{}
	dryRun := boolFlag(command, "dry-run")
	result := output.VerbResult{Kind: "asset", DryRun: dryRun, Details: map[string]any{}}

	if err := content.ValidateEntityPath(project, args[0]); err != nil {
		return err
	}
	if err := output.RequireConfirmation(output.ConfirmArgs{VerbLabel: "removal", Kind: "asset", Identity: args[0], Yes: boolFlag(command, "yes"), Force: boolFlag(command, "force")}, stderr, nil); err != nil {
		return err
	}
	if _, err := service.Remove(project, args[0], boolFlag(command, "force"), dryRun); err != nil {
		return err
	}
	result.Verb, result.Identity = output.Remove, args[0]
	result.Details["removed"] = true

	return renderProjectVerb(stdout, &view, result)
}
