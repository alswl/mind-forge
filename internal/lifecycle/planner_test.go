package lifecycle

import (
	"errors"
	"os"
	"testing"

	"github.com/alswl/mind-forge/internal/output"
)

func TestPlanPreviewDeterministicHashesAndMissingParent(t *testing.T) {
	before := Snapshot{"a": {Kind: "file", Bytes: []byte("old"), Mode: 0o644}, "retained": {Kind: "directory", Mode: 0o755}}
	after := Snapshot{"b": {Kind: "file", Bytes: []byte("new"), Mode: 0o600}, "retained": {Kind: "directory", Mode: 0o755}}
	plan, err := PlanFromSnapshots("/fixture", "rename", []string{"."}, before, after)
	if err != nil {
		t.Fatal(err)
	}
	preview, err := plan.Preview()
	if err != nil || len(preview) != 2 || preview[0].Path != "a" || preview[0].Action != "remove" || preview[0].BeforeHash == nil || preview[0].AfterHash != nil || preview[1].Action != "create" {
		t.Fatalf("preview: %#v (%v)", preview, err)
	}
	before["a"] = Entry{Kind: "file", Bytes: []byte("later"), Mode: 0o644}
	if string(plan.Before["a"].Bytes) != "old" {
		t.Fatal("caller mutation changed an existing plan")
	}
	if _, err := PlanFromSnapshots("/fixture", "invalid", []string{"."}, Snapshot{}, Snapshot{"missing/child": {Kind: "file", Mode: 0o644}}); err == nil {
		t.Fatal("plan without watched parent accepted")
	}
}

func TestRelocationPreservesPayloadAndRefusesOverwrite(t *testing.T) {
	before := Snapshot{"old": {Kind: "directory", Mode: 0o711}, "old/a": {Kind: "file", Bytes: []byte{0, 255}, Mode: 0o600}}
	newPath := "new/container"
	after, err := RelocateSnapshot(before, map[string]*string{"old": &newPath})
	if err != nil {
		t.Fatal(err)
	}
	if err := EnsureParents(after); err != nil {
		t.Fatal(err)
	}
	if after["new/container"].Mode != 0o711 || string(after["new/container/a"].Bytes) != string([]byte{0, 255}) || after["new"].Kind != "directory" {
		t.Fatal(after)
	}
	if _, exists := before["old"]; !exists {
		t.Fatal("pure relocation mutated input")
	}
	conflict := cloneSnapshot(before)
	conflict["new/container"] = Entry{Kind: "directory", Mode: 0o755}
	if _, err := RelocateSnapshot(conflict, map[string]*string{"old": &newPath}); err == nil {
		t.Fatal("occupied destination overwritten")
	}
	removed, err := RelocateSnapshot(before, map[string]*string{"old": nil})
	if err != nil || len(removed) != 0 {
		t.Fatalf("subtree removal: %#v (%v)", removed, err)
	}
}

func TestPlanBindsRootDigestAndBeforeInventoryWithoutWrites(t *testing.T) {
	root := t.TempDir()
	plan, err := NewPlan(root, "create", []string{"."}, Snapshot{}, Snapshot{"a": {Kind: "file", Bytes: []byte("payload"), Mode: 0o644}})
	if err != nil {
		t.Fatal(err)
	}
	if err := plan.VerifyRoot(root); err != nil {
		t.Fatal(err)
	}
	if err := plan.VerifyBefore(Snapshot{}); err != nil {
		t.Fatal(err)
	}
	var typed *output.Error
	if err := plan.VerifyBefore(Snapshot{"unexpected": {Kind: "file"}}); !errors.As(err, &typed) || typed.Kind != "stale_plan" {
		t.Fatalf("stale inventory accepted: %v", err)
	}
	if err := plan.VerifyRoot(t.TempDir()); !errors.As(err, &typed) || typed.Kind != "wrong_worktree" {
		t.Fatalf("wrong root accepted: %v", err)
	}
	plan.Operation = "changed"
	if err := plan.VerifyRoot(root); !errors.As(err, &typed) || typed.Kind != "invalid_plan" {
		t.Fatalf("tampered plan accepted: %v", err)
	}
	if entries, err := os.ReadDir(root); err != nil || len(entries) != 0 {
		t.Fatal("planning wrote repository files")
	}
}
