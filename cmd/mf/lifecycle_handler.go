package main

import (
	"io"

	"github.com/alswl/mind-forge/internal/output"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/lifecycle"
	"github.com/spf13/cobra"
)

// Recovery commands own their locks through Engine; Run deliberately skips its
// normal write lock and recovery guard for these public endpoints.

func executeLifecyclePlan(command *cobra.Command, application *app.Context, engine lifecycle.Service, plan lifecycle.Plan, stderr io.Writer) (map[string]any, error) {
	writes, err := plan.Preview()
	if err != nil {
		return nil, err
	}
	dryRun := boolFlag(command, "dry-run")
	if !dryRun {
		if err := confirmLifecyclePlan(command, plan, writes, stderr); err != nil {
			return nil, err
		}
	}
	return engine.Execute(command.Context(), application.RepoRoot, plan, dryRun, boolFlag(command, "emit-plan"), false)
}

func confirmLifecyclePlan(command *cobra.Command, plan lifecycle.Plan, writes []lifecycle.WritePreview, stderr io.Writer) error {
	for _, write := range writes {
		if write.Action == "remove" {
			if err := output.RequireConfirmation(output.ConfirmArgs{VerbLabel: "applying lifecycle removals", Kind: "plan", Identity: plan.Digest, Yes: boolFlag(command, "yes")}, stderr, nil); err != nil {
				return err
			}
			break
		}
	}
	return nil
}
