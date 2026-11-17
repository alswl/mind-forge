package cli

import (
	"io"
	"os"

	"github.com/alswl/mind-forge/internal/app"
	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/spf13/cobra"
)

func configCommand(command *cobra.Command, application *app.Context, stdout io.Writer) error {
	format := stringFlag(command, "output-format")
	if command.Name() == "schema" {
		if application.Quiet {
			return nil
		}
		return output.RenderRaw(stdout, application.Format, app.ConfigSchema(format))
	}
	if command.Name() == "terminal" {
		return terminalCommand(application, stdout)
	}
	config := model.DefaultMindConfig()
	if command.Name() != "default" {
		var err error
		config, err = app.EffectiveConfig(application.CWD, application.RepoRoot)
		if err != nil {
			return err
		}
	}
	if command.Name() == "show" && application.Format == "json" {
		if application.Quiet {
			return nil
		}
		return output.RenderSuccess(stdout, application.Format, config)
	}
	content, err := app.SerializeConfig(config, format)
	if err != nil {
		return err
	}
	if command.Name() == "generate" {
		path := stringFlag(command, "out")
		// Preserve create/truncate behavior, including an existing file's mode.
		if err := os.WriteFile(path, []byte(content), 0o666); err != nil {
			return err
		}
		if application.Quiet {
			return nil
		}
		return output.RenderSuccess(stdout, application.Format, map[string]string{"path": path})
	}
	if application.Quiet {
		return nil
	}
	return output.RenderRaw(stdout, application.Format, content)
}
