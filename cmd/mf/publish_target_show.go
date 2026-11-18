package main

import (
	"io"
	"strings"

	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/pkg/common/textcase"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/publish"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("publish target show", "publish target", newPublishTargetShowCommand, func(e commandExecution) error {
		return executePublishTargetShow(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newPublishTargetShowCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "show"}
	return configureCommand(command, item, handler)
}

func executePublishTargetShow(c *cobra.Command, a *app.Context, args []string, stdout, stderr io.Writer) error {

	return executePublishTargetShowDetails(c, a, args, stdout)
}

func executePublishTargetShowDetails(c *cobra.Command, a *app.Context, args []string, stdout io.Writer) error {
	report, err := publish.Discover(a.RepoRoot)
	if err != nil {
		return err
	}

	for _, item := range report.Publishers {
		p := item.View
		if !textcase.ASCIIEqualFold(p.Name, args[0]) {
			continue
		}
		if a.Quiet {
			return nil
		}
		label, description := "", ""
		if p.Label != nil {
			label = *p.Label
		}
		if p.Description != nil {
			description = *p.Description
		}
		required := "-"
		if len(p.RequiredInputs) > 0 {
			required = strings.Join(p.RequiredInputs, ", ")
		}
		block := output.ShowBlock{Kind: "publish_target", Identity: p.Name, Fields: []output.ShowField{
			{Label: "Name", Value: output.ShowValue{Text: p.Name}}, {Label: "Type", Value: output.ShowValue{Text: p.TargetType}}, {Label: "Status", Value: output.ShowValue{Text: p.Status}},
			{Label: "Label", Value: output.ShowValue{Text: label, Optional: true, Present: p.Label != nil}}, {Label: "Description", Value: output.ShowValue{Text: description, Optional: true, Present: p.Description != nil}},
			{Label: "Source", Value: output.ShowValue{Text: p.SourcePath, Path: true}}, {Label: "Required inputs", Value: output.ShowValue{Text: required}},
		}}
		if a.Format == "json" {
			data, err := jsonObject(p)
			if err != nil {
				return err
			}
			return output.RenderSuccess(stdout, "json", output.ShowEnvelope(block, data))
		}
		return output.RenderRaw(stdout, "text", output.RenderShow(block, output.DisplayOptions{RepoRoot: a.RepoRoot, Hyperlinks: commandProfile(stdout, a).Hyperlinks}))
	}
	return output.Usage("publish target '"+args[0]+"' not found", "use `mf publish target list` to see available targets")
}
