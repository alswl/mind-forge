package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("project rename", "project", newProjectRenameCommand, func(e commandExecution) error {
		return executeProjectRename(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newProjectRenameCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "rename"}
	return configureCommand(command, item, handler)
}

func executeProjectRename(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
	root := application.RepoRoot
	dryRun := boolFlag(command, "dry-run")

	old := args[0]
	result := output.VerbResult{Verb: output.Rename, Kind: "project", Identity: args[1], OldIdentity: &old, DryRun: dryRun, Details: map[string]any{}}
	if !dryRun {
		report, err := (content.ProjectOperations{}).RenameProject(command.Context(), root, args[0], args[1])
		if err != nil {
			return err
		}
		result.Identity = report.NewName
		result.Path = &report.To
		result.Details = map[string]any{"from": report.From, "to": report.To}
	}
	return renderProjectVerb(stdout, application, result)
}
