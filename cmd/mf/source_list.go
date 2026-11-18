package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/source"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("source list", "source", newSourceListCommand, func(e commandExecution) error {
		return executeSourceList(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newSourceListCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "list"}
	return configureCommand(command, item, handler)
}

func executeSourceList(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {

	project, err := app.ResolveProject(application.RepoRoot, optionalFlag(command, "project"), application.CWD, false)
	if err != nil {
		return err
	}
	view := *application
	view.RepoRoot = project

	return executeSourceListDetails(command, &view, args, stdout)
}

func executeSourceListDetails(command *cobra.Command, application *app.Context, args []string, stdout io.Writer) error {
	project := application.RepoRoot
	kind := optionalFlag(command, "type")
	if kind != nil && *kind == "auto" {
		return output.Usage("--type auto is not valid for listing; specify a concrete type", "use --type pdf, file, rss, or web")
	}
	items, err := source.List(project, optionalFlag(command, "filter"), kind)
	if err != nil {
		return err
	}

	if application.Quiet {
		return nil
	}
	if application.Format == "json" {
		objects := make([]map[string]any, 0, len(items))
		for _, item := range items {
			object, err := jsonObject(item)
			if err != nil {
				return err
			}
			object["identity"] = item.Name
			objects = append(objects, object)
		}
		return output.RenderSuccess(stdout, "json", output.JSONCollection("sources", objects))
	}
	view := output.List{Headers: []string{"NAME", "TYPE", "LOCATION"}, Plural: "sources"}
	for _, item := range items {
		row := output.TextCells(item.Name, item.Kind, sourceLocation(item))
		row[2].Path = true
		view.Rows = append(view.Rows, row)
	}
	options := output.ListOptionsFromProfile(commandProfile(stdout, application), boolFlag(command, "no-headers"), boolFlag(command, "no-trunc"))
	options.RepoRoot = project
	return output.RenderRaw(stdout, "text", output.RenderList(view, options))
}
