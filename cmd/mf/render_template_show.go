package main

import (
	"io"
	"strconv"
	"strings"

	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/pkg/common/textcase"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/render"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("render template show", "render template", newRenderTemplateShowCommand, func(e commandExecution) error {
		return executeRenderTemplateShow(e.Command, e.Application, e.Args, e.Stdout)
	})
}

func newRenderTemplateShowCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "show"}
	return configureCommand(command, item, handler)
}

func executeRenderTemplateShow(c *cobra.Command, a *app.Context, args []string, stdout io.Writer) error {

	templates, err := render.Templates(a.RepoRoot)
	if err != nil {
		return err
	}

	for _, item := range templates {
		if !textcase.ASCIIEqualFold(item.Info.Name, args[0]) {
			continue
		}
		if a.Quiet {
			return nil
		}
		path := ""
		if item.Info.Path != nil {
			path = *item.Info.Path
		}
		preview := strings.Split(strings.TrimSuffix(item.Body, "\n"), "\n")
		if len(preview) > 10 {
			preview = preview[:10]
		}
		block := output.ShowBlock{Kind: "render_template", Identity: item.Info.Name, Fields: []output.ShowField{
			{Label: "Name", Value: output.ShowValue{Text: item.Info.Name}},
			{Label: "Label", Value: output.ShowValue{Text: item.Info.Label}},
			{Label: "Description", Value: output.ShowValue{Text: item.Info.Description}},
			{Label: "Source", Value: output.ShowValue{Text: item.Info.Source}},
			{Label: "Path", Value: output.ShowValue{Text: path, Optional: true, Present: item.Info.Path != nil}},
			{Label: "Default", Value: output.ShowValue{Text: strconv.FormatBool(item.Info.Default)}},
		}, Sections: []output.ShowSection{{Heading: "Preview", Fields: []output.ShowField{{Label: "Body", Value: output.ShowValue{Text: strings.Join(preview, "\n")}}}}}}
		if a.Format == "json" {
			data, err := jsonObject(item.Info)
			if err != nil {
				return err
			}
			data["path"] = item.Info.Path
			return output.RenderSuccess(stdout, "json", output.ShowEnvelope(block, data))
		}
		return output.RenderRaw(stdout, "text", output.RenderShow(block, output.DisplayOptions{RepoRoot: a.RepoRoot, Hyperlinks: commandProfile(stdout, a).Hyperlinks}))
	}
	return output.Usage("template '"+args[0]+"' not found", "use `mf render template list` to see available templates")
}
