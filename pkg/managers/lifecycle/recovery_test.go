package lifecycle

import (
	"errors"
	"os"
	"path/filepath"
	"testing"
)

func TestUnpublishedPreparationBoundariesRetainOrRecoverIntent(t *testing.T) {
	for _, stage := range []string{"directory", "backup", "publish"} {
		for _, rollback := range []bool{false, true} {
			t.Run(stage+map[bool]string{false: "/resume", true: "/rollback"}[rollback], func(t *testing.T) {
				root, plan := transactionFixture(t)
				engine := Engine{Checkpoint: func(name string) error {
					if name == "prepare:"+stage {
						return errComponentFault
					}
					return nil
				}}
				if _, err := engine.Apply(root, plan); !errors.Is(err, errComponentFault) {
					t.Fatal(err)
				}
				assertSnapshot(t, root, plan.Before)
				archived, err := (Engine{}).RecoverUnpublished(root, rollback)
				if err != nil {
					t.Fatal(err)
				}
				if stage == "directory" || stage == "backup" && rollback {
					if !archived {
						t.Fatal("unpublished preparation was not archived")
					}
					assertSnapshot(t, root, plan.Before)
					if again, err := (Engine{}).RecoverUnpublished(root, rollback); err != nil || !again {
						t.Fatalf("archive receipt lost: %t (%v)", again, err)
					}
					return
				}
				if archived {
					t.Fatal("complete intent was discarded")
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
	}
}

func TestRecoveryKeepsUnknownPreparationEvidence(t *testing.T) {
	root, plan := transactionFixture(t)
	engine := Engine{Checkpoint: func(name string) error {
		if name == "prepare:directory" {
			return errComponentFault
		}
		return nil
	}}
	if _, err := engine.Apply(root, plan); !errors.Is(err, errComponentFault) {
		t.Fatal(err)
	}
	path := filepath.Join(root, preparing, "unknown")
	if err := os.WriteFile(path, []byte("retained"), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := (Engine{}).RecoverUnpublished(root, false); err == nil {
		t.Fatal("unknown preparation archived silently")
	}
	if raw, err := os.ReadFile(path); err != nil || string(raw) != "retained" {
		t.Fatal("unknown evidence lost")
	}
	assertSnapshot(t, root, plan.Before)
}

func TestOriginalPlanRepairsOnlyUnchangedInitialPreparation(t *testing.T) {
	for _, rollback := range []bool{false, true} {
		t.Run(map[bool]string{false: "resume", true: "rollback"}[rollback], func(t *testing.T) {
			root, plan := transactionFixture(t)
			if err := os.MkdirAll(filepath.Join(root, state), 0o700); err != nil {
				t.Fatal(err)
			}
			partial := []byte(`{"plan":`)
			if err := os.WriteFile(filepath.Join(root, journalPath+".next"), partial, 0o600); err != nil {
				t.Fatal(err)
			}
			if _, err := (Engine{}).RecoverPreparation(root, plan, rollback); err != nil {
				t.Fatal(err)
			}
			want := plan.After
			if rollback {
				want = plan.Before
			}
			assertSnapshot(t, root, want)
			evidence := filepath.Join(root, namespace, "lifecycle-preparation-"+Digest(partial)+".json")
			if raw, err := os.ReadFile(evidence); err != nil || string(raw) != string(partial) {
				t.Fatal("initial partial journal evidence lost")
			}
		})
	}
}

func TestCompletedTransactionsDoNotRewriteOrChangeDirection(t *testing.T) {
	root, plan := transactionFixture(t)
	if _, err := (Engine{}).Apply(root, plan); err != nil {
		t.Fatal(err)
	}
	info, err := os.Stat(filepath.Join(root, "a.md"))
	if err != nil {
		t.Fatal(err)
	}
	fs := &faultFilesystem{root: root}
	if _, err := (Engine{Filesystem: fs}).Recover(root, false); err != nil {
		t.Fatal(err)
	}
	if _, err := (Engine{Filesystem: fs}).Apply(root, plan); err != nil {
		t.Fatal(err)
	}
	if len(fs.trace) != 0 {
		t.Fatalf("completed operation wrote again: %#v", fs.trace)
	}
	if _, err := (Engine{}).Recover(root, true); err == nil {
		t.Fatal("completed transaction reversed direction")
	}
	after, err := os.Stat(filepath.Join(root, "a.md"))
	if err != nil || !info.ModTime().Equal(after.ModTime()) {
		t.Fatal("completed retry changed author mtime")
	}
	assertSnapshot(t, root, plan.After)
}

func TestRecoveryOfActiveNextRequiresDurableBackupBeforeAuthors(t *testing.T) {
	root, plan := transactionFixture(t)
	if err := os.MkdirAll(filepath.Join(root, state), 0o700); err != nil {
		t.Fatal(err)
	}
	raw, err := CanonicalJSON(Journal{Plan: plan, Phase: "prepared"})
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, journalPath+".next"), raw, 0o600); err != nil {
		t.Fatal(err)
	}
	fs := &faultFilesystem{root: root, fail: "fsync-file:" + journalPath + ".next"}
	if _, err := (Engine{Filesystem: fs}).Recover(root, false); !errors.Is(err, errComponentFault) {
		t.Fatalf("backup sync error was swallowed: %v", err)
	}
	assertSnapshot(t, root, plan.Before)
	if _, err := (Engine{}).Recover(root, false); err != nil {
		t.Fatal(err)
	}
	assertSnapshot(t, root, plan.After)
}

func TestRuntimeMetadataRejectsSymlinksAndHardlinkedJournals(t *testing.T) {
	for _, kind := range []string{"symlink-namespace", "symlink-payload", "hardlink-journal"} {
		t.Run(kind, func(t *testing.T) {
			root, plan := transactionFixture(t)
			outside := t.TempDir()
			if kind == "symlink-namespace" {
				if err := os.Symlink(outside, filepath.Join(root, namespace)); err != nil {
					t.Fatal(err)
				}
			} else {
				if err := os.MkdirAll(filepath.Join(root, state), 0o700); err != nil {
					t.Fatal(err)
				}
				if kind == "symlink-payload" {
					if err := os.Symlink(outside, filepath.Join(root, state, "payload")); err != nil {
						t.Fatal(err)
					}
				} else {
					file := filepath.Join(outside, "journal")
					if err := os.WriteFile(file, []byte("retained"), 0o600); err != nil {
						t.Fatal(err)
					}
					if err := os.Link(file, filepath.Join(root, journalPath)); err != nil {
						t.Fatal(err)
					}
				}
			}
			if _, err := (Engine{}).Apply(root, plan); err == nil {
				t.Fatal("unsafe runtime metadata accepted")
			}
			assertSnapshot(t, root, plan.Before)
		})
	}
}

func TestInitialPreparationRepairRefusesChangedAuthorsAndCompleteBackup(t *testing.T) {
	for _, complete := range []bool{false, true} {
		t.Run(map[bool]string{false: "changed-authors", true: "complete-backup"}[complete], func(t *testing.T) {
			root, plan := transactionFixture(t)
			if err := os.MkdirAll(filepath.Join(root, state), 0o700); err != nil {
				t.Fatal(err)
			}
			raw := []byte(`{"plan":`)
			if complete {
				var err error
				raw, err = CanonicalJSON(Journal{Plan: plan, Phase: "prepared"})
				if err != nil {
					t.Fatal(err)
				}
			}
			path := filepath.Join(root, journalPath+".next")
			if err := os.WriteFile(path, raw, 0o600); err != nil {
				t.Fatal(err)
			}
			if !complete {
				if err := os.WriteFile(filepath.Join(root, "a.md"), []byte("external edit"), 0o640); err != nil {
					t.Fatal(err)
				}
			}
			if _, err := (Engine{}).RecoverPreparation(root, plan, false); err == nil {
				t.Fatal("initial preparation repair bypassed conflict/complete-backup guard")
			}
			if got, err := os.ReadFile(path); err != nil || string(got) != string(raw) {
				t.Fatal("refused repair modified journal evidence")
			}
			if complete {
				assertSnapshot(t, root, plan.Before)
			} else {
				if got, err := os.ReadFile(filepath.Join(root, "a.md")); err != nil || string(got) != "external edit" {
					t.Fatal("refused repair overwrote later author edit")
				}
			}
		})
	}
}
