package lifecycle

import (
	"bytes"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"testing"

	"github.com/alswl/mind-forge/internal/output"
)

func transactionFixture(t *testing.T) (string, Plan) {
	t.Helper()
	root := t.TempDir()
	before := Snapshot{"a.md": {Kind: "file", Bytes: []byte("before"), Mode: 0o640}, "old": {Kind: "directory", Mode: 0o711}, "old/note": {Kind: "file", Bytes: []byte{0, 255, 3}, Mode: 0o600}}
	if err := os.Mkdir(filepath.Join(root, "old"), 0o711); err != nil {
		t.Fatal(err)
	}
	for name, entry := range before {
		if entry.Kind == "file" {
			if err := os.WriteFile(filepath.Join(root, name), entry.Bytes, os.FileMode(entry.Mode)); err != nil {
				t.Fatal(err)
			}
		}
	}
	after := Snapshot{"a.md": {Kind: "file", Bytes: []byte("after"), Mode: 0o600}, "new": {Kind: "directory", Mode: 0o750}, "new/note": {Kind: "file", Bytes: []byte{0, 255, 3}, Mode: 0o600}}
	plan, err := NewPlan(root, "component-fixture", []string{"."}, before, after)
	if err != nil {
		t.Fatal(err)
	}
	return root, plan
}

func assertSnapshot(t *testing.T, root string, expected Snapshot) {
	t.Helper()
	actual, err := SnapshotFiles(root, []string{"."})
	if err != nil {
		t.Fatal(err)
	}
	if len(actual) != len(expected) {
		t.Fatalf("inventory size %d, want %d: %#v", len(actual), len(expected), actual)
	}
	for name, want := range expected {
		got, exists := actual[name]
		if !sameEntry(got, exists, want, true) {
			t.Fatalf("inventory differs at %s: %#v, want %#v", name, got, want)
		}
	}
}

func TestTransactionCanRecoverEachComponentDurableBoundary(t *testing.T) {
	for rollbackIndex := 0; rollbackIndex < 2; rollbackIndex++ {
		rollback := rollbackIndex == 1
		for stage := 0; stage <= 5; stage++ {
			t.Run(fmt.Sprintf("rollback=%t/stage=%d", rollback, stage), func(t *testing.T) {
				root, plan := transactionFixture(t)
				engine := Engine{Checkpoint: func(name string) error {
					if name == fmt.Sprintf("apply:%d", stage) {
						return errComponentFault
					}
					return nil
				}}
				if _, err := engine.Apply(root, plan); !errors.Is(err, errComponentFault) {
					t.Fatalf("durable boundary not reached: %v", err)
				}
				journal, err := (Engine{}).Recover(root, rollback)
				if err != nil {
					t.Fatal(err)
				}
				want, phase := plan.After, "complete"
				if rollback {
					want, phase = plan.Before, "rolled_back"
				}
				if journal.Phase != phase {
					t.Fatalf("terminal phase: %s", journal.Phase)
				}
				assertSnapshot(t, root, want)
				if _, err := (Engine{}).Recover(root, rollback); err != nil {
					t.Fatalf("repeat recovery: %v", err)
				}
				assertSnapshot(t, root, want)
			})
		}
	}
}

var errComponentFault = errors.New("injected lifecycle component failure")

func TestTransactionRejectsLaterEditsAndKeepsJournal(t *testing.T) {
	root, plan := transactionFixture(t)
	engine := Engine{Checkpoint: func(name string) error {
		if name == "apply:0" {
			return errComponentFault
		}
		return nil
	}}
	if _, err := engine.Apply(root, plan); !errors.Is(err, errComponentFault) {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, "a.md"), []byte("external edit"), 0o640); err != nil {
		t.Fatal(err)
	}
	var typed *output.Error
	if _, err := (Engine{}).Recover(root, false); !errors.As(err, &typed) || typed.Kind != "recovery_conflict" {
		t.Fatalf("external edit accepted: %v", err)
	}
	journal, err := (Engine{}).ReadJournal(root)
	if err != nil || journal == nil || journal.Phase != "prepared" {
		t.Fatalf("conflict lost recovery evidence: %#v (%v)", journal, err)
	}
}

func TestTransactionValidationIsReadOnlyAndStalePlanDoesNotPrepare(t *testing.T) {
	root, plan := transactionFixture(t)
	if err := (Engine{}).ValidatePlan(root, plan); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(filepath.Join(root, ".mind-forge")); !os.IsNotExist(err) {
		t.Fatal("validation created transaction state")
	}
	if err := os.WriteFile(filepath.Join(root, "a.md"), []byte("later"), 0o640); err != nil {
		t.Fatal(err)
	}
	var typed *output.Error
	if _, err := (Engine{}).Apply(root, plan); !errors.As(err, &typed) || typed.Kind != "stale_plan" {
		t.Fatalf("stale plan accepted: %v", err)
	}
	if _, err := os.Stat(filepath.Join(root, ".mind-forge")); !os.IsNotExist(err) {
		t.Fatal("rejected stale plan created transaction state")
	}
}

func TestRecoveryConflictsPreserveAuthorsAndJournalInBothDirections(t *testing.T) {
	for _, rollback := range []bool{false, true} {
		for _, conflict := range []string{"bytes", "mode", "kind", "missing", "unknown"} {
			t.Run(fmt.Sprintf("rollback=%t/%s", rollback, conflict), func(t *testing.T) {
				root, _ := interruptedTransaction(t)
				path := filepath.Join(root, "a.md")
				var err error
				switch conflict {
				case "bytes":
					err = os.WriteFile(path, []byte("external edit"), 0o600)
				case "mode":
					err = os.Chmod(path, 0o644)
				case "kind":
					if err = os.Remove(path); err == nil {
						err = os.Mkdir(path, 0o700)
					}
				case "missing":
					err = os.Remove(path)
				case "unknown":
					err = os.WriteFile(filepath.Join(root, "unknown"), []byte("external"), 0o600)
				}
				if err != nil {
					t.Fatal(err)
				}
				snapshot, err := SnapshotFiles(root, []string{"."})
				if err != nil {
					t.Fatal(err)
				}
				journalPath := filepath.Join(root, journalPath)
				journalBefore, err := os.ReadFile(journalPath)
				if err != nil {
					t.Fatal(err)
				}
				for attempt := 0; attempt < 2; attempt++ {
					var typed *output.Error
					if _, err := (Engine{}).Recover(root, rollback); !errors.As(err, &typed) || typed.Kind != "recovery_conflict" {
						t.Fatalf("conflict accepted: %v", err)
					}
					assertSnapshot(t, root, snapshot)
					journalAfter, err := os.ReadFile(journalPath)
					if err != nil || !bytes.Equal(journalBefore, journalAfter) {
						t.Fatalf("journal evidence changed: %v", err)
					}
				}
			})
		}
	}
}

func TestApplyRequiresLockBeforeAnyTransactionWrites(t *testing.T) {
	root, plan := transactionFixture(t)
	fs := &faultFilesystem{root: root}
	attempts := 0
	engine := Engine{Filesystem: fs, AcquireLock: func(path string) (io.Closer, error) {
		attempts++
		if path != root {
			t.Fatalf("locked wrong root: %s", path)
		}
		return nil, errComponentFault
	}}
	if _, err := engine.Apply(root, plan); !errors.Is(err, errComponentFault) {
		t.Fatalf("lock refusal lost: %v", err)
	}
	if attempts != 1 || len(fs.trace) != 0 {
		t.Fatalf("attempts=%d, writes=%v", attempts, fs.trace)
	}
	assertSnapshot(t, root, plan.Before)
	if _, err := os.Stat(filepath.Join(root, namespace)); !os.IsNotExist(err) {
		t.Fatalf("lock refusal created runtime state: %v", err)
	}
}
