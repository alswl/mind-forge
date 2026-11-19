package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/source"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("source update", "source", newSourceUpdateCommand, func(e commandExecution) error {
		return executeSourceUpdate(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newSourceUpdateCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "update"}
	return configureCommand(command, item, handler)
}

func executeSourceUpdate(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {

	project, err := app.ResolveProject(application.RepoRoot, optionalFlag(command, "project"), application.CWD, false)
	if err != nil {
		return err
	}
	view := *application
	view.RepoRoot = project
	service := source.Service{}
	dryRun := boolFlag(command, "dry-run")
	result := output.VerbResult{Kind: "source", DryRun: dryRun, Details: map[string]any{}}
	name, err := source.ResolveUpdateRequest(project, args[0])
	if err != nil {
		return err
	}

	rename, url := optionalFlag(command, "rename"), optionalFlag(command, "url")
	changes := map[string]any{}
	if rename != nil {
		changes["rename"] = map[string]any{"from": args[0], "to": *rename}
	}
	if url != nil {
		changes["url"] = map[string]any{"to": *url}
	}
	result.Verb, result.Identity = output.Update, name
	result.Details["changes"] = changes
	if rename != nil {
		result.OldIdentity = &name
		result.Identity = *rename
	}
	if !dryRun {
		item, err := service.Update(project, name, rename, url)
		if err != nil {
			return err
		}
		result.Details["source"] = item
	}

	return renderProjectVerb(stdout, &view, result)
}
