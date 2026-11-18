package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/pkg/common/model"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("config show", "config", newConfigShowCommand, func(e commandExecution) error {
		return executeConfigShow(e.Command, e.Application, e.Stdout)
	})
}

func newConfigShowCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "show"}
	return configureCommand(command, item, handler)
}

func executeConfigShow(command *cobra.Command, application *app.Context, stdout io.Writer) error {
	format := stringFlag(command, "output-format")

	var config model.MindConfig
	{

		var err error
		config, err = app.EffectiveConfig(application.CWD, application.RepoRoot)
		if err != nil {
			return err
		}
	}

	if application.Format == "json" {
		if application.Quiet {
			return nil
		}
		return output.RenderSuccess(stdout, application.Format, config)
	}
	content, err := app.SerializeConfig(config, format)
	if err != nil {
		return err
	}

	if application.Quiet {
		return nil
	}
	return output.RenderRaw(stdout, application.Format, content)
}
