package main

import (
	"fmt"
	"io"
	"strings"

	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/pkg/common/model"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/source"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("source clean", "source", newSourceCleanCommand, func(e commandExecution) error {
		return executeSourceClean(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newSourceCleanCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "clean"}
	return configureCommand(command, item, handler)
}

func executeSourceClean(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {

	project, err := app.ResolveProject(application.RepoRoot, optionalFlag(command, "project"), application.CWD, false)
	if err != nil {
		return err
	}
	view := *application
	view.RepoRoot = project
	service := source.Service{}
	dryRun := boolFlag(command, "dry-run")

	var report model.SourceIndexReport

	report, err = service.Clean(project, dryRun)

	if err != nil {
		return err
	}

	if application.Quiet {
		return nil
	}
	if application.Format == "json" {
		return output.RenderSuccess(stdout, "json", report)
	}
	text := "No dirty sources."
	if len(report.Removed) > 0 {
		prefix := ""
		if dryRun {
			prefix = "[dry-run] "
		}
		lines := []string{}
		for _, entry := range report.Removed {
			lines = append(lines, fmt.Sprintf("%s- removed: %s (%s)", prefix, entry.Name, entry.Kind))
		}
		lines = append(lines, fmt.Sprintf("%skept: %d entries", prefix, report.KeptCount))
		text = strings.Join(lines, "\n")
	}
	return output.RenderRaw(stdout, "text", text)
}
