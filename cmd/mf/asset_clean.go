package main

import (
	"fmt"
	"io"
	"strings"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/asset"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("asset clean", "asset", newAssetCleanCommand, func(e commandExecution) error {
		return executeAssetClean(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newAssetCleanCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "clean"}
	return configureCommand(command, item, handler)
}

func executeAssetClean(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
	project, err := app.ResolveProject(application.RepoRoot, optionalFlag(command, "project"), application.CWD, false)
	if err != nil {
		return err
	}
	view := *application
	view.RepoRoot = project
	service := asset.Service{}
	dryRun := boolFlag(command, "dry-run")

	{

		report, err := service.Clean(project, dryRun)
		if err != nil {
			return err
		}
		if application.Quiet {
			return nil
		}
		if application.Format == "json" {
			return output.RenderSuccess(stdout, "json", report)
		}
		prefix := ""
		if dryRun {
			prefix = "[dry-run] "
		}
		text := prefix + "No stale entries found."
		if report.RemovedCount > 0 {
			lines := []string{fmt.Sprintf("%sCleaned %d stale entries from index.", prefix, report.RemovedCount)}
			for _, path := range report.StaleEntries {
				lines = append(lines, "  removed: "+path)
			}
			text = strings.Join(lines, "\n")
		}
		return output.RenderRaw(stdout, "text", text)
	}
}
