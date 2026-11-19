package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
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
	outputs, err := content.ListOutputsRequest(application.RepoRoot, application.CWD, optionalFlag(command, "project"), selector)
	if err != nil {
		return err
	}
	data := map[string]any{"outputs": outputs}
	if application.Quiet {
		return nil
	}
	if application.Format == "json" {
		return output.RenderSuccess(stdout, "json", data)
	}
	return renderMindText(data, application, stdout)
}
