package lifecycle

import (
	"errors"
	"testing"

	"github.com/alswl/mind-forge/internal/output"
)

func TestPartialRecoveryAcceptsBeforeAfterAndRejectsLaterEdits(t *testing.T) {
	before := Snapshot{"a.md": {Kind: "file", Bytes: []byte("before"), Mode: 0o644}}
	after := Snapshot{"b.md": {Kind: "file", Bytes: []byte("after"), Mode: 0o644}}
	plan := Plan{Before: before, After: after}
	for _, current := range []Snapshot{before, after, {}, {"a.md": before["a.md"], "b.md": after["b.md"]}} {
		if err := plan.VerifyPartial(current); err != nil {
			t.Fatalf("valid intermediate state rejected: %#v: %v", current, err)
		}
	}
	for _, current := range []Snapshot{
		{"a.md": {Kind: "file", Bytes: []byte("later edit"), Mode: 0o644}},
		{"new.md": {Kind: "file", Bytes: []byte("unknown"), Mode: 0o644}},
		{"a.md": {Kind: "file", Bytes: []byte("before"), Mode: 0o600}},
	} {
		if err := plan.VerifyPartial(current); err == nil {
			t.Errorf("conflicting recovery state accepted: %#v", current)
		}
	}
}

func TestPlanRejectsUnsafeAndUnwatchedWrites(t *testing.T) {
	for _, path := range []string{"../outside", "/absolute", ".git/config", ".mind-forge/journal", "p/../q", "unwatched/file"} {
		plan := Plan{Version: 1, Watches: []string{"project"}, Before: Snapshot{}, After: Snapshot{path: {Kind: "file", Mode: 0o644}}}
		if err := plan.ValidateShape(); err == nil {
			t.Errorf("unsafe/unwatched path accepted: %s", path)
		}
	}
	plan := Plan{Version: 1, Watches: []string{"."}, After: Snapshot{"A": {Kind: "file"}, "a": {Kind: "file"}}}
	if err := plan.ValidateShape(); err == nil {
		t.Fatal("case-folded collision accepted")
	}
}

func TestPlanValidationErrorsAreTypedAndDeterministic(t *testing.T) {
	plan := Plan{Version: 1, Watches: []string{"."}, After: Snapshot{"z/child": {Kind: "file"}, "a/child": {Kind: "file"}}}
	for i := 0; i < 20; i++ {
		var typed *output.Error
		err := plan.ValidateShape()
		if !errors.As(err, &typed) || typed.Kind != "invalid_plan" || typed.ExitCode() != 2 || typed.Message != "missing parent for a/child" {
			t.Fatalf("validation error: %v", err)
		}
	}
}

func TestPlanCaseCollisionsUseFrozenRustUnicodeLowercasing(t *testing.T) {
	for _, names := range [][2]string{{"İ", "i\u0307"}, {"ΟΣ", "ος"}} {
		plan := Plan{Version: 1, Watches: []string{"."}, After: Snapshot{names[0]: {Kind: "file"}, names[1]: {Kind: "file"}}}
		if err := plan.ValidateShape(); err == nil {
			t.Fatalf("Unicode path collision accepted: %q and %q", names[0], names[1])
		}
	}
}

func TestRelativePathMatchesFrozenRustNormalComponentRules(t *testing.T) {
	for _, name := range []string{"a//b", "a/./b", "a/.", "a/"} {
		if err := relativePath(name); err != nil {
			t.Fatalf("normal components rejected: %s (%v)", name, err)
		}
	}
	for _, name := range []string{"./a", ".", "a/../b"} {
		if err := relativePath(name); err == nil {
			t.Fatalf("nonnormal components accepted: %s", name)
		}
	}
}

func TestPartialRecoveryRejectsHybridAndMissingCommonEntries(t *testing.T) {
	before := Entry{Kind: "file", Bytes: []byte("before"), Mode: 0o640}
	after := Entry{Kind: "file", Bytes: []byte("after"), Mode: 0o600}
	directory := Entry{Kind: "directory", Mode: 0o750}
	plan := Plan{Before: Snapshot{"common": before, "removed": before, "directory": directory}, After: Snapshot{"common": after, "added": after, "directory": directory}}
	for mask := 0; mask < 8; mask++ {
		current := Snapshot{"common": before, "directory": directory}
		if mask&1 != 0 {
			current["common"] = after
		}
		if mask&2 != 0 {
			current["removed"] = before
		}
		if mask&4 != 0 {
			current["added"] = after
		}
		if err := plan.VerifyPartial(current); err != nil {
			t.Fatalf("independent before/after choices %d rejected: %v", mask, err)
		}
	}
	for _, invalid := range []struct {
		name   string
		entry  Entry
		absent bool
	}{
		{name: "common", absent: true},
		{name: "directory", absent: true},
		{name: "common", entry: Entry{Kind: "file", Bytes: before.Bytes, Mode: after.Mode}},
		{name: "common", entry: Entry{Kind: "file", Bytes: after.Bytes, Mode: before.Mode}},
		{name: "common", entry: Entry{Kind: "directory", Mode: before.Mode}},
		{name: "directory", entry: Entry{Kind: "directory", Mode: 0o700}},
		{name: "unknown", entry: before},
	} {
		current := Snapshot{"common": before, "directory": directory, "removed": before}
		if invalid.absent {
			delete(current, invalid.name)
		} else {
			current[invalid.name] = invalid.entry
		}
		var typed *output.Error
		err := plan.VerifyPartial(current)
		if !errors.As(err, &typed) || typed.Kind != "recovery_conflict" || typed.ExitCode() != 2 || typed.Message != "later edit or unknown entry: "+invalid.name {
			t.Fatalf("invalid state %+v: %v", invalid, err)
		}
	}
}
