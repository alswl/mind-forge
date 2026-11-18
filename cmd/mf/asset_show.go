package main

import (
	"fmt"
	"io"
	"strings"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/asset"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("asset show", "asset", newAssetShowCommand, func(e commandExecution) error {
		return executeAssetShow(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newAssetShowCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "show"}
	return configureCommand(command, item, handler)
}

func executeAssetShow(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
	project, err := app.ResolveProject(application.RepoRoot, optionalFlag(command, "project"), application.CWD, false)
	if err != nil {
		return err
	}
	view := *application
	view.RepoRoot = project

	return executeAssetShowDetails(command, &view, args, stdout)
}

func executeAssetShowDetails(command *cobra.Command, application *app.Context, args []string, stdout io.Writer) error {
	project := application.RepoRoot

	if err := content.ValidateEntityPath(project, args[0]); err != nil {
		return err
	}
	item, err := asset.Show(project, args[0])
	if err != nil {
		return err
	}
	if application.Quiet {
		return nil
	}
	block := output.ShowBlock{Kind: "asset", Identity: item.Path, Fields: []output.ShowField{{Label: "Name", Value: output.ShowValue{Text: item.Name}}, {Label: "Type", Value: output.ShowValue{Text: item.Kind}}, {Label: "Path", Value: output.ShowValue{Text: item.Path, Path: true}}, {Label: "Size", Value: output.ShowValue{Text: fmt.Sprintf("%d bytes", item.Size)}}}}
	if item.Hash != "" {
		block.Fields = append(block.Fields, output.ShowField{Label: "Hash", Value: output.ShowValue{Text: item.Hash}})
	}
	if len(item.Tags) > 0 {
		block.Fields = append(block.Fields, output.ShowField{Label: "Tags", Value: output.ShowValue{Text: strings.Join(item.Tags, ", ")}})
	}
	block.Fields = append(block.Fields, output.ShowField{Label: "Added", Value: output.ShowValue{Text: item.AddedAt}})
	if application.Format == "json" {
		extra, err := jsonObject(item)
		if err != nil {
			return err
		}
		return output.RenderSuccess(stdout, "json", output.ShowEnvelope(block, extra))
	}
	profile := commandProfile(stdout, application)
	return output.RenderRaw(stdout, "text", output.RenderShow(block, output.DisplayOptions{RepoRoot: project, Hyperlinks: profile.Hyperlinks}))
}
