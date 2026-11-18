package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("project remove", "project", newProjectRemoveCommand, func(e commandExecution) error {
		return executeProjectRemove(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newProjectRemoveCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "remove"}
	return configureCommand(command, item, handler)
}

func executeProjectRemove(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
	root := application.RepoRoot
	dryRun := boolFlag(command, "dry-run")
	force := boolFlag(command, "force")

	label := "removal"

	if err := output.RequireConfirmation(output.ConfirmArgs{VerbLabel: label, Kind: "project", Identity: args[0], Yes: boolFlag(command, "yes"), Force: force}, stderr, nil); err != nil {
		return err
	}
	result := output.VerbResult{Verb: output.Remove, Kind: "project", Identity: args[0], DryRun: dryRun, Details: map[string]any{}}
	if force {
		missing, err := content.ProjectTargetMissing(root, args[0])
		if err != nil {
			return err
		}
		if missing {
			key := "removed"

			result.Details = map[string]any{key: false, "reason": "not found"}
			return renderProjectVerb(stdout, application, result)
		}
	}

	path, err := content.RemoveProject(root, args[0], dryRun)
	if err != nil {
		return err
	}
	result.Path = &path
	result.Details = map[string]any{"removed": true, "path": path}
	return renderProjectVerb(stdout, application, result)
}
