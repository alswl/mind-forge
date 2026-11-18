package main

import (
	"io"
	"os"

	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/pkg/common/model"
	app "github.com/alswl/mind-forge/pkg/config"
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

	var config model.MindConfig
	{

		var err error
		config, err = app.EffectiveConfig(application.CWD, application.RepoRoot)
		if err != nil {
			return err
		}
	}

	content, err := app.SerializeConfig(config, format)
	if err != nil {
		return err
	}

	path := stringFlag(command, "out")

	if err := os.WriteFile(path, []byte(content), 0o666); err != nil {
		return err
	}
	if application.Quiet {
		return nil
	}
	return output.RenderSuccess(stdout, application.Format, map[string]string{"path": path})
}
