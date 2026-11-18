package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("config schema", "config", newConfigSchemaCommand, func(e commandExecution) error {
		return executeConfigSchema(e.Command, e.Application, e.Stdout)
	})
}

func newConfigSchemaCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "schema"}
	return configureCommand(command, item, handler)
}

func executeConfigSchema(command *cobra.Command, application *app.Context, stdout io.Writer) error {
	format := stringFlag(command, "output-format")

	if application.Quiet {
		return nil
	}
	return output.RenderRaw(stdout, application.Format, app.ConfigSchema(format))
}
