package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/pkg/common/model"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/source"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("source index", "source", newSourceIndexCommand, func(e commandExecution) error {
		return executeSourceIndex(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newSourceIndexCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "index"}
	return configureCommand(command, item, handler)
}

func executeSourceIndex(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {

	project, err := app.ResolveProject(application.RepoRoot, optionalFlag(command, "project"), application.CWD, false)
	if err != nil {
		return err
	}
	view := *application
	view.RepoRoot = project
	service := source.Service{}
	dryRun := boolFlag(command, "dry-run")
	result := output.VerbResult{Kind: "source", DryRun: dryRun, Details: map[string]any{}}

	var report model.SourceIndexReport

	report, err = service.Reconcile(project, dryRun)

	if err != nil {
		return err
	}

	result.Verb = output.Index
	result.Details = map[string]any{"added": report.Added, "removed": report.Removed, "kept_count": report.KeptCount, "scanned_count": uint64(len(report.Added)+len(report.Removed)) + report.KeptCount}
	if application.Format == "json" {
		if application.Quiet {
			return nil
		}
		data := result.Details
		data["kind"], data["dry_run"] = "source", dryRun
		return output.RenderSuccess(stdout, "json", data)
	}

	return renderProjectVerb(stdout, &view, result)
}
