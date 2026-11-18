package main

import (
	"fmt"
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/asset"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("asset update", "asset", newAssetUpdateCommand, func(e commandExecution) error {
		return executeAssetUpdate(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newAssetUpdateCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "update"}
	return configureCommand(command, item, handler)
}

func executeAssetUpdate(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
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

		if len(args) == 0 && (optionalFlag(command, "set-url") != nil || optionalFlag(command, "channel") != nil) {
			url, channel := "", ""
			if value := optionalFlag(command, "set-url"); value != nil {
				url = *value
			}
			if value := optionalFlag(command, "channel"); value != nil {
				channel = *value
			}
			if err := service.SetPublishURL(project, url, channel, dryRun); err != nil {
				return err
			}
			if application.Quiet {
				return nil
			}
			if application.Format == "json" {
				return output.RenderSuccess(stdout, "json", map[string]any{"url": url, "channel": channel})
			}
			return output.RenderRaw(stdout, "text", fmt.Sprintf("Set publish URL: %s (channel: %s)", url, channel))
		}
		all := boolFlag(command, "all")
		if len(args) > 0 && all {
			return output.Usage("<PATH> and --all are mutually exclusive", "provide either <PATH> or --all, not both")
		}
		if len(args) == 0 && !all {
			return output.Usage("provide <PATH> or --all", "provide a path to update a single asset, or --all to update all")
		}
		if all {
			items, err := service.UpdateAll(project, dryRun)
			if err != nil {
				return err
			}
			return renderAssetUpdates(stdout, &view, items, dryRun)
		}
		report, err := service.UpdateOne(project, application.CWD, args[0], dryRun)
		if err != nil {
			return err
		}
		result.Verb, result.Identity, result.Path = output.Update, report.Path, &report.Path
		changes := map[string]any{}
		if report.Changed {
			changes["size"] = map[string]any{"from": report.OldSize, "to": report.NewSize}
			changes["hash"] = map[string]any{"from": report.OldHash, "to": report.NewHash}
		}
		result.Details = map[string]any{"changes": changes, "changed": report.Changed}
	}

	return renderProjectVerb(stdout, &view, result)
}
