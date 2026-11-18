package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/render"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("render", "", newRenderCommand, func(e commandExecution) error {
		return executeRender(e.Command, e.Application, e.Args, e.Stdout)
	})
}

func newRenderCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "render"}
	return configureCommand(command, item, handler)
}

func executeRender(c *cobra.Command, a *app.Context, args []string, stdout io.Writer) error {

	var article *string
	if len(args) > 0 {
		article = &args[0]
	}
	template := "report"
	if selected := optionalFlag(c, "template"); selected != nil {
		template = *selected
	}
	generated, err := render.GenerateRequest(a.RepoRoot, a.CWD, optionalFlag(c, "project"), article, template, stringFlag(c, "html-form"))
	if err != nil {
		return err
	}

	if a.Quiet {
		return nil
	}
	if a.Format == "json" {
		data, err := jsonObject(generated)
		if err != nil {
			return err
		}
		data["article"] = article
		return output.RenderSuccess(stdout, "json", data)
	}
	return output.RenderRaw(stdout, "text", generated.Prompt)
}
