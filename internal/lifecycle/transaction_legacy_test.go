package lifecycle

import (
	"bytes"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/internal/store"
)

func legacyTransactionFixture(t *testing.T) (string, Plan) {
	t.Helper()
	root := t.TempDir()
	if err := os.MkdirAll(filepath.Join(root, "project/old"), 0o755); err != nil {
		t.Fatal(err)
	}
	for name, raw := range map[string][]byte{
		"project/old/block.md":  []byte("# Title\n\nBody\n"),
		"project/old/image.bin": {0, 255, 1, 128},
		"project/mind.yaml":     []byte("content_layout: category\n"),
	} {
		legacyTransactionWrite(t, root, name, raw)
	}
	before, err := SnapshotFiles(root, []string{"project"})
	if err != nil {
		t.Fatal(err)
	}
	after := Snapshot{}
	for name, entry := range before {
		after[strings.ReplaceAll(name, "project/old", "project/new")] = entry
	}
	entry := after["project/mind.yaml"]
	entry.Bytes = []byte("content_layout: bundle\n")
	after["project/mind.yaml"] = entry
	plan, err := NewPlan(root, "test move", []string{"project"}, before, after)
	if err != nil {
		t.Fatal(err)
	}
	return root, plan
}

func legacyTransactionWrite(t *testing.T, root, name string, raw []byte) {
	t.Helper()
	if err := os.WriteFile(filepath.Join(root, name), raw, 0o644); err != nil {
		t.Fatal(err)
	}
}

func legacyTransactionKind(t *testing.T, err error, want string) {
	t.Helper()
	var typed *output.Error
	if !errors.As(err, &typed) || typed.Kind != want {
		t.Fatalf("error=%v, want kind %s", err, want)
	}
}

func legacyTransactionInterrupt(t *testing.T, root string, plan Plan, stage int) {
	t.Helper()
	_, err := (Engine{Checkpoint: func(name string) error {
		if name == fmt.Sprintf("apply:%d", stage) {
			return errComponentFault
		}
		return nil
	}}).Apply(root, plan)
	if !errors.Is(err, errComponentFault) {
		t.Fatalf("stage %d not reached: %v", stage, err)
	}
}

func legacyTransactionRead(t *testing.T, root, name string) []byte {
	t.Helper()
	raw, err := os.ReadFile(filepath.Join(root, name))
	if err != nil {
		t.Fatal(err)
	}
	return raw
}

func legacyTransactionAbsent(t *testing.T, root, name string) {
	t.Helper()
	if _, err := os.Lstat(filepath.Join(root, name)); !os.IsNotExist(err) {
		t.Fatalf("%s exists or cannot be inspected: %v", name, err)
	}
}

func TestLegacyTransactionInitialPreparation(t *testing.T) {
	for _, rollback := range []bool{false, true} {
		for _, partial := range []bool{false, true} {
			t.Run(fmt.Sprintf("rollback=%t/partial=%t", rollback, partial), func(t *testing.T) {
				root, plan := legacyTransactionFixture(t)
				if err := os.MkdirAll(filepath.Join(root, state), 0o700); err != nil {
					t.Fatal(err)
				}
				raw := []byte(`{"plan":`)
				if partial {
					legacyTransactionWrite(t, root, journalPath+".next", raw)
				}
				if err := (Engine{}).CheckRecovery(root); err == nil {
					t.Fatal("incomplete preparation did not gate reads")
				}
				journal, err := (Engine{}).RecoverPreparation(root, plan, rollback)
				if err != nil {
					t.Fatal(err)
				}
				want, phase := plan.After, "complete"
				if rollback {
					want, phase = plan.Before, "rolled_back"
				}
				if journal.Phase != phase {
					t.Fatalf("phase %s, want %s", journal.Phase, phase)
				}
				assertSnapshot(t, root, want)
				if partial {
					evidence := namespace + "/lifecycle-preparation-" + Digest(raw) + ".json"
					if !bytes.Equal(legacyTransactionRead(t, root, evidence), raw) {
						t.Fatal("partial journal evidence lost")
					}
				}
			})
		}
	}
}

func TestLegacyTransactionPreparationConflicts(t *testing.T) {
	for _, unknown := range []bool{false, true} {
		t.Run(fmt.Sprint(unknown), func(t *testing.T) {
			root, plan := legacyTransactionFixture(t)
			if err := os.MkdirAll(filepath.Join(root, state), 0o700); err != nil {
				t.Fatal(err)
			}
			kind := "recovery_conflict"
			if unknown {
				legacyTransactionWrite(t, root, state+"/payload", []byte("unknown"))
				kind = "invalid_journal"
			} else {
				legacyTransactionWrite(t, root, "project/old/block.md", []byte("later edit"))
			}
			_, err := (Engine{}).RecoverPreparation(root, plan, true)
			legacyTransactionKind(t, err, kind)
		})
	}
}

func TestLegacyTransactionDirectoryStagingPermissions(t *testing.T) {
	for _, rollback := range []bool{false, true} {
		t.Run(fmt.Sprint(rollback), func(t *testing.T) {
			root, plan := legacyTransactionFixture(t)
			legacyTransactionInterrupt(t, root, plan, 0)
			staging := filepath.Join(root, state, "directory")
			if err := os.Mkdir(staging, 0o711); err != nil {
				t.Fatal(err)
			}
			if err := os.Chmod(staging, 0o711); err != nil {
				t.Fatal(err)
			}
			if _, err := (Engine{}).Recover(root, rollback); err != nil {
				t.Fatal(err)
			}
			want := plan.After
			if rollback {
				want = plan.Before
			}
			assertSnapshot(t, root, want)
		})
	}
	root, plan := legacyTransactionFixture(t)
	after := cloneSnapshot(plan.After)
	after["project/new"] = Entry{Kind: "directory", Mode: 0o500}
	_, err := NewPlan(root, "permissions", plan.Watches, plan.Before, after)
	legacyTransactionKind(t, err, "unsupported_permissions")
}

func TestLegacyTransactionPreviewInventory(t *testing.T) {
	root, plan := legacyTransactionFixture(t)
	preview, err := plan.Preview()
	if err != nil {
		t.Fatal(err)
	}
	found := false
	for _, p := range preview {
		if p.Path == "project/mind.yaml" && p.BeforeHash != nil && p.AfterHash != nil {
			found = true
		}
	}
	if !found {
		t.Fatal("preview lacks before and after hashes")
	}
	legacyTransactionAbsent(t, root, namespace)
	assertSnapshot(t, root, plan.Before)
	legacyTransactionWrite(t, root, "project/new-review.md", []byte("late inbound reference"))
	_, err = (Engine{}).Apply(root, plan)
	legacyTransactionKind(t, err, "stale_plan")
	legacyTransactionAbsent(t, root, journalPath)
	if info, err := os.Stat(filepath.Join(root, "project/old/image.bin")); err != nil || !info.Mode().IsRegular() {
		t.Fatal("stale plan lost binary input", err)
	}
}

func TestLegacyTransactionEveryApplyStageResumesWithoutRewriting(t *testing.T) {
	_, sample := legacyTransactionFixture(t)
	preview, err := sample.Preview()
	if err != nil {
		t.Fatal(err)
	}
	for stage := 0; stage <= len(preview)+1; stage++ {
		t.Run(fmt.Sprint(stage), func(t *testing.T) {
			root, plan := legacyTransactionFixture(t)
			legacyTransactionInterrupt(t, root, plan, stage)
			journal, err := (Engine{}).Recover(root, false)
			if err != nil {
				t.Fatal(err)
			}
			if journal.Phase != "complete" {
				t.Fatal(journal.Phase)
			}
			assertSnapshot(t, root, plan.After)
			saved := legacyTransactionRead(t, root, journalPath)
			fs := &faultFilesystem{root: root}
			if _, err := (Engine{Filesystem: fs}).Recover(root, false); err != nil {
				t.Fatal(err)
			}
			if _, err := (Engine{Filesystem: fs}).Apply(root, plan); err != nil {
				t.Fatal(err)
			}
			if !bytes.Equal(saved, legacyTransactionRead(t, root, journalPath)) || len(fs.trace) != 0 {
				t.Fatalf("terminal retry rewrote journal or authors: %v", fs.trace)
			}
		})
	}
}

func TestLegacyTransactionEveryUnfinishedStageRollsBack(t *testing.T) {
	_, sample := legacyTransactionFixture(t)
	preview, err := sample.Preview()
	if err != nil {
		t.Fatal(err)
	}
	for stage := 0; stage <= len(preview); stage++ {
		t.Run(fmt.Sprint(stage), func(t *testing.T) {
			root, plan := legacyTransactionFixture(t)
			legacyTransactionInterrupt(t, root, plan, stage)
			legacyTransactionKind(t, (Engine{}).CheckRecovery(root), "recovery_required")
			journal, err := (Engine{}).Recover(root, true)
			if err != nil {
				t.Fatal(err)
			}
			if journal.Phase != "rolled_back" {
				t.Fatal(journal.Phase)
			}
			assertSnapshot(t, root, plan.Before)
			saved := legacyTransactionRead(t, root, journalPath)
			fs := &faultFilesystem{root: root}
			if _, err := (Engine{Filesystem: fs}).Recover(root, true); err != nil {
				t.Fatal(err)
			}
			if !bytes.Equal(saved, legacyTransactionRead(t, root, journalPath)) || len(fs.trace) != 0 {
				t.Fatalf("rollback retry rewrote evidence or authors: %v", fs.trace)
			}
		})
	}
}

func TestLegacyTransactionRollbackCanBeInterrupted(t *testing.T) {
	_, sample := legacyTransactionFixture(t)
	preview, err := sample.Preview()
	if err != nil {
		t.Fatal(err)
	}
	for stage := 1; stage <= len(preview); stage++ {
		t.Run(fmt.Sprint(stage), func(t *testing.T) {
			root, plan := legacyTransactionFixture(t)
			legacyTransactionInterrupt(t, root, plan, len(preview))
			_, err := (Engine{Checkpoint: func(name string) error {
				if name == fmt.Sprintf("apply:%d", stage) {
					return errComponentFault
				}
				return nil
			}}).Recover(root, true)
			if !errors.Is(err, errComponentFault) {
				t.Fatalf("rollback stage %d not reached: %v", stage, err)
			}
			if _, err := (Engine{}).Recover(root, true); err != nil {
				t.Fatal(err)
			}
			assertSnapshot(t, root, plan.Before)
		})
	}
}

func TestLegacyTransactionProtectsLaterEditsAndEntries(t *testing.T) {
	for _, rollback := range []bool{false, true} {
		for _, addition := range []bool{false, true} {
			t.Run(fmt.Sprintf("rollback=%t/addition=%t", rollback, addition), func(t *testing.T) {
				root, plan := legacyTransactionFixture(t)
				legacyTransactionInterrupt(t, root, plan, 0)
				name, raw := "project/old/block.md", []byte("later authored edit")
				if addition {
					name, raw = "project/additional.md", []byte("later addition")
				}
				legacyTransactionWrite(t, root, name, raw)
				current, err := SnapshotFiles(root, plan.Watches)
				if err != nil {
					t.Fatal(err)
				}
				_, err = (Engine{}).Recover(root, rollback)
				legacyTransactionKind(t, err, "recovery_conflict")
				assertSnapshot(t, root, current)
			})
		}
	}
}

func TestLegacyTransactionWriterAndWorktreeRefusals(t *testing.T) {
	root, plan := legacyTransactionFixture(t)
	lock, err := store.AcquireWriteLock(root)
	if err != nil {
		t.Fatal(err)
	}
	defer lock.Close()
	_, err = (Engine{}).Apply(root, plan)
	legacyTransactionKind(t, err, "write_locked")
	_, err = (Engine{}).Apply(t.TempDir(), plan)
	legacyTransactionKind(t, err, "wrong_worktree")
}

func TestLegacyTransactionPlanAndJournalTampering(t *testing.T) {
	root, plan := legacyTransactionFixture(t)
	plan.Operation = "tampered"
	_, err := (Engine{}).Apply(root, plan)
	legacyTransactionKind(t, err, "invalid_plan")
	legacyTransactionAbsent(t, root, namespace)
	root, plan = legacyTransactionFixture(t)
	legacyTransactionInterrupt(t, root, plan, 0)
	journal, err := (Engine{}).ReadJournal(root)
	if err != nil {
		t.Fatal(err)
	}
	journal.Plan.Operation = "tampered"
	raw, err := CanonicalJSON(journal)
	if err != nil {
		t.Fatal(err)
	}
	legacyTransactionWrite(t, root, journalPath, raw)
	_, err = (Engine{}).Recover(root, false)
	legacyTransactionKind(t, err, "invalid_journal")
}

func TestLegacyTransactionRejectsLinksBeforeBackups(t *testing.T) {
	for _, hard := range []bool{false, true} {
		t.Run(fmt.Sprint(hard), func(t *testing.T) {
			root, plan := legacyTransactionFixture(t)
			var err error
			if hard {
				err = os.Link(filepath.Join(root, "project/old/block.md"), filepath.Join(root, "project/link.md"))
			} else {
				err = os.Symlink("old/block.md", filepath.Join(root, "project/link.md"))
			}
			if err != nil {
				t.Fatal(err)
			}
			_, err = (Engine{}).Apply(root, plan)
			legacyTransactionKind(t, err, "unsafe_path")
			legacyTransactionAbsent(t, root, namespace)
		})
	}
}

func TestLegacyTransactionRecoversDurableNextImage(t *testing.T) {
	root, plan := legacyTransactionFixture(t)
	legacyTransactionInterrupt(t, root, plan, 0)
	if err := os.Rename(filepath.Join(root, journalPath), filepath.Join(root, journalPath+".next")); err != nil {
		t.Fatal(err)
	}
	if _, err := (Engine{}).Recover(root, false); err != nil {
		t.Fatal(err)
	}
	assertSnapshot(t, root, plan.After)
}
