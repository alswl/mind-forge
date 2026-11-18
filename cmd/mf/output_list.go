package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/pkg/common/model"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("output list", "output", newOutputListCommand, func(e commandExecution) error {
		return executeOutputList(e.Command, e.Application, e.Args, e.Stdout)
	})
}

func newOutputListCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "list"}
	return configureCommand(command, item, handler)
}

func executeOutputList(command *cobra.Command, application *app.Context, args []string, stdout io.Writer) error {

	var selector *string
	if len(args) > 0 {
		selector = &args[0]
	}
	project, err := content.ProjectFor(application.RepoRoot, application.CWD, optionalFlag(command, "project"), selector)
	if err != nil {
		return err
	}
	root := application.RepoRoot
	var data map[string]any

	var minds []model.Mind
	if len(args) > 0 {
		var mind model.Mind
		mind, err = content.ResolveMind(root, project, args[0])
		minds = []model.Mind{mind}
	} else {
		minds, err = content.Discover(root, project)
	}
	outputs := model.JSONArray[model.Output]{}
	for _, mind := range minds {
		outputs = append(outputs, mind.Outputs...)
	}
	data = map[string]any{"outputs": outputs}

	if err != nil {
		return err
	}
	if application.Quiet {
		return nil
	}
	if application.Format == "json" {
		return output.RenderSuccess(stdout, "json", data)
	}
	return renderMindText(data, application, stdout)
}
