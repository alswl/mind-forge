package main

import (
	"io"
	"path/filepath"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/asset"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("asset new", "asset", newAssetNewCommand, func(e commandExecution) error {
		return executeAssetNew(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newAssetNewCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "new"}
	return configureCommand(command, item, handler)
}

func executeAssetNew(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
	project, err := app.ResolveProject(application.RepoRoot, optionalFlag(command, "project"), application.CWD, true)
	if err != nil {
		return err
	}
	view := *application
	view.RepoRoot = project
	service := asset.Service{}
	dryRun := boolFlag(command, "dry-run")
	result := output.VerbResult{Kind: "asset", DryRun: dryRun, Details: map[string]any{}}
	switch {
	default:

		result.Verb = output.Add
		if dryRun {
			name := filepath.Base(args[0])
			if name == "." || name == "/" {
				name = "unknown"
			}
			if value := optionalFlag(command, "name"); value != nil {
				name = *value
			}
			result.Identity, result.Path = name, &name
			result.Details["source"] = args[0]
			break
		}
		tags, _ := command.Flags().GetStringArray("tag")
		item, err := service.Add(project, application.CWD, args[0], asset.AddOptions{Name: optionalFlag(command, "name"), Tags: tags, Link: boolFlag(command, "link"), Force: boolFlag(command, "force")})
		if err != nil {
			return err
		}
		result.Identity, result.Path = item.Path, &item.Path
		mode := "copy"
		if boolFlag(command, "link") {
			mode = "link"
		}
		result.Details = map[string]any{"name": item.Name, "type": item.Kind, "path": item.Path, "size": item.Size, "hash": item.Hash, "tags": item.Tags, "added_at": item.AddedAt, "mode": mode}
	}

	return renderProjectVerb(stdout, &view, result)
}
