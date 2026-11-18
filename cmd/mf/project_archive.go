package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("project archive", "project", newProjectArchiveCommand, func(e commandExecution) error {
		return executeProjectArchive(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newProjectArchiveCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "archive"}
	return configureCommand(command, item, handler)
}

func executeProjectArchive(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
	root := application.RepoRoot
	dryRun := boolFlag(command, "dry-run")
	force := boolFlag(command, "force")

	label := "archiving"

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
			key := "archived"

			result.Details = map[string]any{key: false, "reason": "not found"}
			return renderProjectVerb(stdout, application, result)
		}
	}

	message := "[dry-run] would archive project: " + args[0]
	result.Details = map[string]any{"archived": true, "action": "archive"}
	if !dryRun {
		report, err := (content.ProjectOperations{}).ArchiveProject(command.Context(), root, args[0])
		if err != nil {
			return err
		}
		result.Identity = report.Name
		result.Path = &report.To
		result.Details = map[string]any{"archived": true, "action": "archive", "from": report.From, "to": report.To}
		message = "✓ archived project: " + report.Name
	}
	if application.Quiet {
		return nil
	}
	if application.Format == "json" {
		return output.RenderSuccess(stdout, "json", output.VerbEnvelope(result))
	}
	return output.RenderRaw(stdout, "text", message)
}
