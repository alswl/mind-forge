package main

import (
	"fmt"
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/asset"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("asset list", "asset", newAssetListCommand, func(e commandExecution) error {
		return executeAssetList(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newAssetListCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "list"}
	return configureCommand(command, item, handler)
}

func executeAssetList(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
	project, err := app.ResolveProject(application.RepoRoot, optionalFlag(command, "project"), application.CWD, false)
	if err != nil {
		return err
	}
	view := *application
	view.RepoRoot = project

	return executeAssetListDetails(command, &view, args, stdout)
}

func executeAssetListDetails(command *cobra.Command, application *app.Context, args []string, stdout io.Writer) error {
	project := application.RepoRoot

	items, err := asset.List(project, optionalFlag(command, "filter"), optionalFlag(command, "type"))
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
			object["identity"] = item.Path
			objects = append(objects, object)
		}
		return output.RenderSuccess(stdout, "json", output.JSONCollection("assets", objects))
	}
	view := output.List{Headers: []string{"NAME", "TYPE", "PATH", "SIZE"}, Plural: "assets"}
	for _, item := range items {
		row := output.TextCells(item.Name, item.Kind, item.Path, fmt.Sprint(item.Size))
		row[2].Path = true
		view.Rows = append(view.Rows, row)
	}
	options := output.ListOptionsFromProfile(commandProfile(stdout, application), boolFlag(command, "no-headers"), boolFlag(command, "no-trunc"))
	options.RepoRoot = project
	return output.RenderRaw(stdout, "text", output.RenderList(view, options))
}
