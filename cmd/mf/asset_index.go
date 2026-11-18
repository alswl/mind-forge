package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/asset"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("asset index", "asset", newAssetIndexCommand, func(e commandExecution) error {
		return executeAssetIndex(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newAssetIndexCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "index"}
	return configureCommand(command, item, handler)
}

func executeAssetIndex(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
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

		report, err := service.Reconcile(project, dryRun, boolFlag(command, "refresh-metadata"))
		if err != nil {
			return err
		}
		count := uint64(len(report.Added)+len(report.Removed)) + report.KeptCount
		result.Verb = output.Index
		result.Details = map[string]any{"added": report.Added, "removed": report.Removed, "kept_count": report.KeptCount, "scanned_count": count, "refreshed": report.Refreshed}
		if application.Format == "json" {
			if application.Quiet {
				return nil
			}
			return output.RenderSuccess(stdout, "json", map[string]any{"kind": "asset", "added": report.Added, "removed": report.Removed, "kept_count": report.KeptCount, "scanned_count": count, "dry_run": dryRun, "details": map[string]any{"refreshed": report.Refreshed}})
		}
	}

	return renderProjectVerb(stdout, &view, result)
}
