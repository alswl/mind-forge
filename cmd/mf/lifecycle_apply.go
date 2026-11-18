package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/lifecycle"
	"github.com/spf13/cobra"
)

func init() {
	registerCommand("lifecycle apply", "lifecycle", newLifecycleApplyCommand, func(e commandExecution) error {
		return executeLifecycleApply(e.Command, e.Application, e.Args, e.Stdout, e.Stderr)
	})
}

func newLifecycleApplyCommand(item CommandSpec, handler Handler) (*cobra.Command, error) {
	command := &cobra.Command{Use: "apply"}
	return configureCommand(command, item, handler)
}

func executeLifecycleApply(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {

	return executeLifecycleApplyDetails(command, application, args, stdout, stderr)
}

func executeLifecycleApplyDetails(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
	engine := lifecycle.New()
	root := application.RepoRoot
	data := map[string]any{"kind": "lifecycle"}

	plan, err := lifecycle.LoadPlan(application.CWD, args[0])
	if err != nil {
		return err
	}
	writes, err := plan.Preview()
	if err != nil {
		return err
	}
	dry := boolFlag(command, "dry-run")
	if !dry {
		if err := confirmLifecyclePlan(command, plan, writes, stderr); err != nil {
			return err
		}
	}
	outcome, err := engine.Execute(command.Context(), root, plan, dry, false, true)
	if err != nil {
		return err
	}
	data["digest"] = outcome["digest"]
	if dry {
		data["dry_run"], data["writes"] = true, outcome["writes"]
	} else {
		data["phase"] = outcome["phase"]
	}

	if application.Quiet {
		return nil
	}
	return output.RenderSuccess(stdout, application.Format, data)
}
