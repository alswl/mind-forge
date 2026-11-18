package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/pkg/common/model"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("config default", "config", newConfigDefaultCommand, func(e commandExecution) error {
		return executeConfigDefault(e.Command, e.Application, e.Stdout)
	})
}

func newConfigDefaultCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "default"}
	return configureCommand(command, item, handler)
}

func executeConfigDefault(command *cobra.Command, application *app.Context, stdout io.Writer) error {
	format := stringFlag(command, "output-format")

	config := model.DefaultMindConfig()

	content, err := app.SerializeConfig(config, format)
	if err != nil {
		return err
	}

	if application.Quiet {
		return nil
	}
	return output.RenderRaw(stdout, application.Format, content)
}
