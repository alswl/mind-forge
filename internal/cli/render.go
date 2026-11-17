package cli

import (
	"io"
	"path/filepath"
	"strconv"
	"strings"

	"github.com/alswl/mind-forge/internal/app"
	"github.com/alswl/mind-forge/internal/content"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/internal/render"
	"github.com/alswl/mind-forge/internal/textcase"
	"github.com/spf13/cobra"
)

func renderCommand(c *cobra.Command, a *app.Context, args []string, stdout io.Writer) error {
	if c.HasSubCommands() && c.Name() != "render" {
		return c.Help()
	}
	templates, err := render.Templates(a.RepoRoot)
	if err != nil {
		return err
	}
	if c.Name() == "list" {
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
	if c.Name() == "show" {
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
	project, err := app.ResolveProject(a.RepoRoot, optionalFlag(c, "project"), a.CWD, false)
	if err != nil {
		return err
	}
	config, err := content.ConfigFor(project)
	if err != nil {
		return err
	}
	layout, err := app.ResolveAndValidateLayout(project, config)
	if err != nil {
		return err
	}
	var article *string
	if len(args) > 0 {
		article = &args[0]
	}
	template := "report"
	if flag := optionalFlag(c, "template"); flag != nil {
		template = *flag
	}
	generated, err := render.Generate(project, filepath.Base(project), layout.BuildOutput, config.Build.Format, article, template, stringFlag(c, "html-form"), templates)
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
