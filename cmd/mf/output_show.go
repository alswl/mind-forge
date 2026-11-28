package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/content"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("output show", "output", newOutputShowCommand, func(e commandExecution) error {
		return executeOutputShow(e.Command, e.Application, e.Args, e.Stdout)
	})
}

func newOutputShowCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "show"}
	return configureCommand(command, item, handler)
}

func executeOutputShow(command *cobra.Command, application *app.Context, args []string, stdout io.Writer) error {
	project, err := content.ProjectFor(application.RepoRoot, application.CWD, optionalFlag(command, "project"), &args[0])
	if err != nil {
		return err
	}
	item, err := content.ResolveOutput(application.RepoRoot, project, args[0])
	if err != nil {
		return err
	}
	if boolFlag(command, "handoff") {
		view, err := content.ReadOutputHandoff(command.Context(), project, item)
		if err != nil {
			return err
		}
		return renderOutputHandoff(view, application, stdout)
	}
	if application.Quiet {
		return nil
	}
	data, err := jsonObject(item)
	if err != nil {
		return err
	}
	if application.Format == "json" {
		return output.RenderSuccess(stdout, "json", data)
	}
	return renderMindText(data, application, stdout)
}

func renderOutputHandoff(view content.OutputHandoffView, application *app.Context, stdout io.Writer) error {
	if !application.Quiet {
		data, err := jsonObject(view)
		if err != nil {
			return err
		}
		if application.Format == "json" {
			err = output.RenderSuccess(stdout, "json", data)
		} else {
			handoff := view.Handoff
			data["handoff"] = handoff.Status
			data["locations"] = handoff.Locations
			if len(handoff.Diagnostics) > 0 {
				data["diagnostics"] = handoff.Diagnostics
			}
			if handoff.Record != nil {
				data["decision"] = handoff.Record.Decision
				data["rework"] = handoff.Record.Rework
				data["open_items"] = handoff.Record.OpenItems
				data["next_step"] = handoff.Record.NextStep
			}
			err = renderMindText(data, application, stdout)
		}
		if err != nil {
			return err
		}
	}
	if view.Handoff.Status == content.HandoffInvalid || view.Handoff.Status == content.HandoffAmbiguous {
		return &commandExit{1}
	}
	return nil
}
