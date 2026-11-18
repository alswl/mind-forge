package main

import (
	"fmt"
	"io"
	"strings"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/publish"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("publish target list", "publish target", newPublishTargetListCommand, func(e commandExecution) error {
		return executePublishTargetList(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newPublishTargetListCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "list"}
	return configureCommand(command, item, handler)
}

func executePublishTargetList(c *cobra.Command, a *app.Context, args []string, stdout, stderr io.Writer) error {

	return executePublishTargetListDetails(c, a, args, stdout)
}

func executePublishTargetListDetails(c *cobra.Command, a *app.Context, args []string, stdout io.Writer) error {
	report, err := publish.Discover(a.RepoRoot)
	if err != nil {
		return err
	}

	if a.Quiet {
		return nil
	}
	if a.Format == "json" {
		items := []any{}
		for _, item := range report.Publishers {
			data, err := jsonObject(item.View)
			if err != nil {
				return err
			}
			data["identity"] = item.View.Name
			items = append(items, data)
		}
		return output.RenderSuccess(stdout, "json", map[string]any{"publish_targets": items, "diagnostics": report.Diagnostics})
	}
	var diagnostics strings.Builder
	for _, d := range report.Diagnostics {
		path, name := "-", "-"
		if d.Path != nil {
			path = *d.Path
		}
		if d.PublisherName != nil {
			name = *d.PublisherName
		}
		fmt.Fprintf(&diagnostics, "[%s] %s (%s): %s\n", d.Kind, path, name, d.Message)
	}
	view := output.List{Headers: []string{"NAME", "TYPE", "STATUS"}, Plural: "publish targets"}
	for _, item := range report.Publishers {
		view.Rows = append(view.Rows, output.TextCells(item.View.Name, item.View.TargetType, item.View.Status))
	}
	opts := output.ListOptionsFromProfile(commandProfile(stdout, a), boolFlag(c, "no-headers"), boolFlag(c, "no-trunc"))
	return output.RenderRaw(stdout, "text", diagnostics.String()+output.RenderList(view, opts))
}
