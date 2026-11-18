package main

import (
	"fmt"
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("project import", "project", newProjectImportCommand, func(e commandExecution) error {
		return executeProjectImport(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newProjectImportCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "import"}
	return configureCommand(command, item, handler)
}

func executeProjectImport(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
	root := application.RepoRoot

	force := boolFlag(command, "force")

	report, err := content.ImportProject(root, args[0], content.ProjectImportOptions{Type: optionalFlag(command, "type"), Sources: optionalFlag(command, "source"), Assets: optionalFlag(command, "assets"), Force: force})
	if err != nil {
		return err
	}
	if application.Quiet {
		return nil
	}
	if application.Format == "json" {
		return output.RenderSuccess(stdout, "json", report)
	}
	text := fmt.Sprintf("Imported project: %s\n  path: %s\n", report.Name, report.Path)
	if report.Scaffolded {
		text += "  mind.yaml: created\n"
	}
	text += fmt.Sprintf("  articles: %d", report.ArticleCount)
	return output.RenderRaw(stdout, "text", text)
}
