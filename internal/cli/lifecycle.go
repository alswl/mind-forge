package cli

import (
	"encoding/json"
	"io"
	"os"
	"path/filepath"

	"github.com/alswl/mind-forge/internal/app"
	"github.com/alswl/mind-forge/internal/content"
	"github.com/alswl/mind-forge/internal/lifecycle"
	"github.com/alswl/mind-forge/internal/output"
	"github.com/spf13/cobra"
)

func lifecycleCommand(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
	switch command.Name() {
	case "apply", "resume", "rollback", "status":
		return lifecycleRecoveryCommand(command, application, args, stdout, stderr)
	}
	var selector *string
	if len(args) > 0 {
		selector = &args[0]
	}
	project, err := content.ProjectFor(application.RepoRoot, application.CWD, optionalFlag(command, "project"), selector)
	if err != nil {
		return err
	}
	operation := content.LifecycleOperation{Kind: command.Name()}
	if selector != nil {
		operation.Identity = *selector
	}
	if len(args) > 1 {
		operation.Name = args[1]
	}
	if target := optionalFlag(command, "to"); target != nil {
		operation.Layout = *target
	}
	if target := optionalFlag(command, "to-project"); target != nil {
		operation.Project = *target
	}
	if target := optionalFlag(command, "to-mind"); target != nil {
		operation.Mind = *target
	}
	plan, err := content.PlanLifecycle(application.RepoRoot, project, operation)
	if err != nil {
		return err
	}
	data, err := executeLifecyclePlan(command, application, lifecycleEngine(), plan, stderr)
	if err != nil {
		return err
	}
	if application.Quiet {
		return nil
	}
	return output.RenderSuccess(stdout, application.Format, data)
}

func loadLifecyclePlan(cwd, name string) (lifecycle.Plan, error) {
	if !filepath.IsAbs(name) {
		name = filepath.Join(cwd, name)
	}
	raw, err := os.ReadFile(name)
	if err != nil {
		return lifecycle.Plan{}, err
	}
	var plan lifecycle.Plan
	err = json.Unmarshal(raw, &plan)
	return plan, err
}

// Recovery commands own their locks through Engine; Run deliberately skips its
// normal write lock and recovery guard for these public endpoints.
func lifecycleRecoveryCommand(command *cobra.Command, application *app.Context, args []string, stdout, stderr io.Writer) error {
	engine := lifecycleEngine()
	root := application.RepoRoot
	data := map[string]any{"kind": "lifecycle"}
	switch command.Name() {
	case "status":
		journal, err := engine.ReadJournal(root)
		if err != nil {
			return err
		}
		if journal == nil {
			data["phase"] = "none"
		} else {
			writes, err := journal.Plan.Preview()
			if err != nil {
				return err
			}
			data["digest"], data["phase"], data["writes"] = journal.Plan.Digest, journal.Phase, writes
		}
	case "apply":
		plan, err := loadLifecyclePlan(application.CWD, args[0])
		if err != nil {
			return err
		}
		writes, err := plan.Preview()
		if err != nil {
			return err
		}
		if boolFlag(command, "dry-run") {
			if err := engine.ValidatePlan(root, plan); err != nil {
				return err
			}
			data["dry_run"], data["digest"], data["writes"] = true, plan.Digest, writes
		} else {
			if err := confirmLifecyclePlan(command, plan, writes, stderr); err != nil {
				return err
			}
			journal, err := engine.Apply(root, plan)
			if err != nil {
				return err
			}
			data["digest"], data["phase"] = journal.Plan.Digest, journal.Phase
		}
	case "resume", "rollback":
		rollback := command.Name() == "rollback"
		var journal lifecycle.Journal
		var err error
		if name := optionalFlag(command, "plan"); name != nil {
			plan, loadError := loadLifecyclePlan(application.CWD, *name)
			if loadError != nil {
				return loadError
			}
			journal, err = engine.RecoverPreparation(root, plan, rollback)
		} else {
			aborted, recoverError := engine.RecoverUnpublished(root, rollback)
			if recoverError != nil {
				return recoverError
			}
			if aborted {
				data["phase"], data["authored_changes"], data["next_action"] = "preparation_aborted", false, "rerun original lifecycle command to retry; partial evidence retained"
			} else {
				journal, err = engine.Recover(root, rollback)
			}
		}
		if err != nil {
			return err
		}
		if data["phase"] == nil {
			data["digest"], data["phase"] = journal.Plan.Digest, journal.Phase
		}
	}
	if application.Quiet {
		return nil
	}
	return output.RenderSuccess(stdout, application.Format, data)
}

func executeLifecyclePlan(command *cobra.Command, application *app.Context, engine lifecycle.Engine, plan lifecycle.Plan, stderr io.Writer) (map[string]any, error) {
	writes, err := plan.Preview()
	if err != nil {
		return nil, err
	}
	dryRun := boolFlag(command, "dry-run")
	data := map[string]any{"kind": "lifecycle", "dry_run": dryRun, "digest": plan.Digest, "writes": writes, "unverified": "references outside this repository"}
	if dryRun {
		if boolFlag(command, "emit-plan") {
			data["plan"] = plan
		}
		return data, nil
	}
	if err := confirmLifecyclePlan(command, plan, writes, stderr); err != nil {
		return nil, err
	}
	journal, err := engine.Apply(application.RepoRoot, plan)
	if err != nil {
		return nil, err
	}
	data["phase"] = journal.Phase
	return data, nil
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
