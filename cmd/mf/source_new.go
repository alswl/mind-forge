package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/source"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("source new", "source", newSourceNewCommand, func(e commandExecution) error {
		return executeSourceNew(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newSourceNewCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "new"}
	return configureCommand(command, item, handler)
}

func executeSourceNew(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {

	project, err := app.ResolveProject(application.RepoRoot, optionalFlag(command, "project"), application.CWD, false)
	if err != nil {
		return err
	}
	view := *application
	view.RepoRoot = project
	service := source.Service{}
	dryRun := boolFlag(command, "dry-run")
	result := output.VerbResult{Kind: "source", DryRun: dryRun, Details: map[string]any{}}
	kind, err := source.InputKind(args[0], optionalFlag(command, "file-kind"), optionalFlag(command, "source-kind"))
	if err != nil {
		return err
	}
	options := source.AddOptions{Name: optionalFlag(command, "name"), Kind: kind, SourceKind: optionalFlag(command, "source-kind"), Link: boolFlag(command, "link"), Force: boolFlag(command, "force"), RegisterOnly: boolFlag(command, "register-only"), DryRun: dryRun}
	if dryRun && !options.RegisterOnly {
		preview := source.PreviewNew(args[0], options.Name)
		name := preview.Name
		result.Verb, result.Identity, result.Path = output.Add, name, &name
		result.Details = map[string]any{"input": preview.Input}
		return renderProjectVerb(stdout, &view, result)
	}
	outcome, err := service.Add(command.Context(), application.RepoRoot, project, application.CWD, args[0], options)
	if err != nil {
		return err
	}
	item := outcome.Source
	result.Verb, result.Identity, result.Path = output.Add, item.Name, item.Path
	result.Details = map[string]any{"name": item.Name, "type": item.Kind, "url": item.URL, "path": item.Path, "added_at": item.AddedAt, "updated_at": item.UpdatedAt, "mode": outcome.Mode, "replaced": outcome.Replaced}
	if outcome.ReplacedLocation != nil {
		result.Details["replaced_location"] = *outcome.ReplacedLocation
	}
	if !application.Quiet && application.Format != "json" && outcome.ReplacedLocation != nil {
		profile := commandProfile(stdout, application)
		text := output.RenderVerb(result, output.DisplayOptions{RepoRoot: project, Hyperlinks: profile.Hyperlinks}) + "\n  replaced registration at " + *outcome.ReplacedLocation
		return output.RenderRaw(stdout, "text", text)
	}

	return renderProjectVerb(stdout, &view, result)
}
