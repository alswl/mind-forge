package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/configuration"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("config generate", "config", newConfigGenerateCommand, func(e commandExecution) error {
		return executeConfigGenerate(e.Command, e.Application, e.Stdout)
	})
}

func newConfigGenerateCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "generate"}
	return configureCommand(command, item, handler)
}

func executeConfigGenerate(command *cobra.Command, application *app.Context, stdout io.Writer) error {
	format := stringFlag(command, "output-format")

	path := stringFlag(command, "out")
	if err := (configuration.Exporter{}).Export(application.CWD, application.RepoRoot, format, path); err != nil {
		return err
	}
	if application.Quiet {
		return nil
	}
	return output.RenderSuccess(stdout, application.Format, map[string]string{"path": path})
}
