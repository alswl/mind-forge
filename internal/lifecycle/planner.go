package lifecycle

import (
	"path"
	"path/filepath"
	"sort"
	"strings"

	"github.com/alswl/mind-forge/internal/store"
)

type WritePreview struct {
	Path       string  `json:"path"`
	BeforeHash *string `json:"before_hash"`
	AfterHash  *string `json:"after_hash"`
	Action     string  `json:"action"`
}

func cloneSnapshot(snapshot Snapshot) Snapshot {
	result := make(Snapshot, len(snapshot))
	for path, entry := range snapshot {
		entry.Bytes = append([]byte(nil), entry.Bytes...)
		result[path] = entry
	}
	return result
}

func PlanFromSnapshots(root, operation string, watches []string, before, after Snapshot) (Plan, error) {
	plan := Plan{Version: 1, Root: root, Operation: operation, Watches: append([]string{}, watches...), Before: cloneSnapshot(before), After: cloneSnapshot(after)}
	if err := plan.ValidateShape(); err != nil {
		return Plan{}, err
	}
	digest, err := plan.ComputeDigest()
	if err != nil {
		return Plan{}, err
	}
	plan.Digest = digest
	return plan, nil
}

func NewPlan(root, operation string, watches []string, before, after Snapshot) (Plan, error) {
	canonical, err := filepath.EvalSymlinks(root)
	if err != nil {
		return Plan{}, err
	}
	canonical, err = filepath.Abs(canonical)
	if err != nil {
		return Plan{}, err
	}
	if err := store.CheckLocalFilesystem(canonical); err != nil {
		return Plan{}, err
	}
	return PlanFromSnapshots(canonical, operation, watches, before, after)
}

func (plan Plan) VerifyRoot(root string) error {
	canonical, err := filepath.EvalSymlinks(root)
	if err != nil {
		return err
	}
	canonical, err = filepath.Abs(canonical)
	if err != nil {
		return err
	}
	if canonical != plan.Root {
		return rejected("wrong_worktree", "plan is bound to a different root")
	}
	if err := store.CheckLocalFilesystem(canonical); err != nil {
		return err
	}
	if err := plan.ValidateShape(); err != nil {
		return err
	}
	digest, err := plan.ComputeDigest()
	if err != nil {
		return err
	}
	if digest != plan.Digest {
		return rejected("invalid_plan", "digest mismatch")
	}
	return nil
}

func (plan Plan) VerifyBefore(current Snapshot) error {
	if len(current) != len(plan.Before) {
		return rejected("stale_plan", "watched inventory changed")
	}
	for name, before := range plan.Before {
		actual, exists := current[name]
		if !sameEntry(actual, exists, before, true) {
			return rejected("stale_plan", "watched inventory changed")
		}
	}
	return nil
}

func (plan Plan) Preview() ([]WritePreview, error) {
	paths := map[string]bool{}
	for path := range plan.Before {
		paths[path] = true
	}
	for path := range plan.After {
		paths[path] = true
	}
	ordered := make([]string, 0, len(paths))
	for path := range paths {
		ordered = append(ordered, path)
	}
	sort.Strings(ordered)
	result := []WritePreview{}
	for _, path := range ordered {
		before, hadBefore := plan.Before[path]
		after, hasAfter := plan.After[path]
		if sameEntry(before, hadBefore, after, hasAfter) {
			continue
		}
		preview := WritePreview{Path: path, Action: "update"}
		if hadBefore {
			hash, err := before.Hash()
			if err != nil {
				return nil, err
			}
			preview.BeforeHash = &hash
		}
		if hasAfter {
			hash, err := after.Hash()
			if err != nil {
				return nil, err
			}
			preview.AfterHash = &hash
		}
		if !hasAfter {
			preview.Action = "remove"
		} else if !hadBefore {
			preview.Action = "create"
		}
		result = append(result, preview)
	}
	return result, nil
}

func Relocated(name string, moves map[string]*string) *string {
	matched := ""
	for source := range moves {
		if name == source || strings.HasPrefix(name, source+"/") {
			if len(source) > len(matched) {
				matched = source
			}
		}
	}
	if matched == "" {
		return &name
	}
	if moves[matched] == nil {
		return nil
	}
	result := *moves[matched] + strings.TrimPrefix(name, matched)
	return &result
}

func RelocateSnapshot(before Snapshot, moves map[string]*string) (Snapshot, error) {
	after := cloneSnapshot(before)
	additions := Snapshot{}
	names := make([]string, 0, len(before))
	for name := range before {
		names = append(names, name)
	}
	sort.Strings(names)
	for _, name := range names {
		target := Relocated(name, moves)
		if target != nil && *target == name {
			continue
		}
		delete(after, name)
		if target == nil {
			continue
		}
		if _, exists := before[*target]; exists {
			return nil, rejected("path_conflict", *target)
		}
		if _, exists := additions[*target]; exists {
			return nil, rejected("path_conflict", *target)
		}
		entry := before[name]
		entry.Bytes = append([]byte(nil), entry.Bytes...)
		additions[*target] = entry
	}
	for name, entry := range additions {
		after[name] = entry
	}
	return after, nil
}

func EnsureParents(after Snapshot) error {
	names := make([]string, 0, len(after))
	for name := range after {
		names = append(names, name)
	}
	sort.Strings(names)
	for _, name := range names {
		for parent := path.Dir(name); parent != "." && parent != "/"; parent = path.Dir(parent) {
			if entry, exists := after[parent]; exists {
				if entry.Kind != "directory" {
					return rejected("path_conflict", parent)
				}
			} else {
				after[parent] = Entry{Kind: "directory", Mode: 0o755}
			}
		}
	}
	return nil
}
