package main

import (
	"io"
	"strings"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/alswl/mind-forge/pkg/services/source"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("source remove", "source", newSourceRemoveCommand, func(e commandExecution) error {
		return executeSourceRemove(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newSourceRemoveCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "remove"}
	return configureCommand(command, item, handler)
}

func executeSourceRemove(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
	if len(args) > 0 && !strings.Contains(args[0], "/") && !strings.HasPrefix(args[0], "sources") {
		if err := output.WarnDeprecation(stderr, "positional NAME", "full PATH (e.g., sources/yuque/foo.md)", !application.Color); err != nil {
			return err
		}
	}
	project, err := app.ResolveProject(application.RepoRoot, optionalFlag(command, "project"), application.CWD, false)
	if err != nil {
		return err
	}
	if err := content.ValidateEntityPath(project, args[0]); err != nil {
		return err
	}
	if err := output.RequireConfirmation(output.ConfirmArgs{VerbLabel: "removal", Kind: "source", Identity: args[0], Yes: boolFlag(command, "yes"), Force: boolFlag(command, "force")}, stderr, nil); err != nil {
		return err
	}
	dryRun := boolFlag(command, "dry-run")
	report, err := (source.Service{}).Remove(project, args[0], boolFlag(command, "keep-file"), boolFlag(command, "force"), dryRun)
	if err != nil {
		return err
	}
	result := output.VerbResult{Verb: output.Remove, Kind: "source", DryRun: dryRun, Identity: report.Source.Name, Path: report.Source.Path, Details: map[string]any{"removed": true}}
	view := *application
	view.RepoRoot = project
	return renderProjectVerb(stdout, &view, result)
}
