package lifecycle

import (
	"context"
	"encoding/json"
	"os"
	"path/filepath"

	"github.com/alswl/mind-forge/pkg/common/task"
	manager "github.com/alswl/mind-forge/pkg/managers/lifecycle"
)

type Plan = manager.Plan
type WritePreview = manager.WritePreview

type Service struct{ engine manager.Engine }

func New() Service { return Service{engine: newEngine()} }

func LoadPlan(cwd, name string) (Plan, error) {
	if !filepath.IsAbs(name) {
		name = filepath.Join(cwd, name)
	}
	raw, err := os.ReadFile(name)
	if err != nil {
		return Plan{}, err
	}
	var plan Plan
	err = json.Unmarshal(raw, &plan)
	return plan, err
}

func (service Service) Status(root string) (map[string]any, error) {
	data := map[string]any{"kind": "lifecycle"}
	journal, err := service.engine.ReadJournal(root)
	if err != nil {
		return nil, err
	}
	if journal == nil {
		data["phase"] = "none"
	} else {
		writes, err := journal.Plan.Preview()
		if err != nil {
			return nil, err
		}
		data["digest"], data["phase"], data["writes"] = journal.Plan.Digest, journal.Phase, writes
	}
	return data, nil
}

func (service Service) Recover(ctx context.Context, root string, rollback bool, plan *Plan) (map[string]any, error) {
	task.Notify(ctx, task.Stage{Operation: "lifecycle", Name: "recovering transaction"})
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	service.engine.Context = ctx
	data := map[string]any{"kind": "lifecycle"}
	var journal manager.Journal
	var err error
	if plan != nil {
		journal, err = service.engine.RecoverPreparation(root, *plan, rollback)
	} else {
		aborted, recoverError := service.engine.RecoverUnpublished(root, rollback)
		if recoverError != nil {
			return nil, recoverError
		}
		if aborted {
			data["phase"], data["authored_changes"], data["next_action"] = "preparation_aborted", false, "rerun original lifecycle command to retry; partial evidence retained"
		} else {
			journal, err = service.engine.Recover(root, rollback)
		}
	}
	if err != nil {
		return nil, err
	}
	if data["phase"] == nil {
		data["digest"], data["phase"] = journal.Plan.Digest, journal.Phase
	}
	return data, nil
}

// Execute binds preview validation and transaction execution to one outcome.
// The caller obtains confirmation before requesting a non-preview execution.
func (service Service) Execute(ctx context.Context, root string, plan Plan, dry, emit, validate bool) (map[string]any, error) {
	task.Notify(ctx, task.Stage{Operation: "lifecycle", Name: "validating plan"})
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	writes, err := plan.Preview()
	if err != nil {
		return nil, err
	}
	data := map[string]any{"kind": "lifecycle", "dry_run": dry, "digest": plan.Digest, "writes": writes, "unverified": "references outside this repository"}
	engine := service.engine
	engine.Context = ctx
	if dry {
		if validate {
			if err := engine.ValidatePlan(root, plan); err != nil {
				return nil, err
			}
		}
		if emit {
			data["plan"] = plan
		}
		return data, nil
	}
	journal, err := engine.Apply(root, plan)
	if err != nil {
		return nil, err
	}
	data["phase"] = journal.Phase
	return data, nil
}
