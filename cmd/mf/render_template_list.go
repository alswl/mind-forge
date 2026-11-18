package main

import (
	"io"
	"strconv"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/render"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("render template list", "render template", newRenderTemplateListCommand, func(e commandExecution) error {
		return executeRenderTemplateList(e.Command, e.Application, e.Args, e.Stdout)
	})
}

func newRenderTemplateListCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "list"}
	return configureCommand(command, item, handler)
}

func executeRenderTemplateList(c *cobra.Command, a *app.Context, args []string, stdout io.Writer) error {

	templates, err := render.Templates(a.RepoRoot)
	if err != nil {
		return err
	}

	if a.Quiet {
		return nil
	}
	if a.Format == "json" {
		items := []any{}
		for _, item := range templates {
			data, err := jsonObject(item.Info)
			if err != nil {
				return err
			}
			data["identity"] = item.Info.Name
			data["path"] = item.Info.Path
			items = append(items, data)
		}
		return output.RenderSuccess(stdout, "json", map[string]any{"templates": items})
	}
	view := output.List{Headers: []string{"NAME", "LABEL", "SOURCE", "DEFAULT"}, Plural: "templates"}
	for _, item := range templates {
		view.Rows = append(view.Rows, output.TextCells(item.Info.Name, item.Info.Label, item.Info.Source, strconv.FormatBool(item.Info.Default)))
	}
	opts := output.ListOptionsFromProfile(commandProfile(stdout, a), boolFlag(c, "no-headers"), boolFlag(c, "no-trunc"))
	return output.RenderRaw(stdout, "text", output.RenderList(view, opts))
}
