package lifecycle

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"testing"
)

type fullConversionFixture struct {
	Subject     string `json:"subject"`
	RootProject bool   `json:"root_project"`
	Plan        Plan   `json:"plan"`
}

func fullConversionFixtures(t *testing.T) []fullConversionFixture {
	t.Helper()
	raw, err := os.ReadFile("testdata/full-conversion-plans.json")
	if err != nil {
		t.Fatal(err)
	}
	var fixtures []fullConversionFixture
	if err := json.Unmarshal(raw, &fixtures); err != nil {
		t.Fatal(err)
	}
	if len(fixtures) != 4 {
		t.Fatal("missing root or subject fixture")
	}
	return fixtures
}

func fullConversionInput(t *testing.T, frozen Plan) (string, Plan) {
	t.Helper()
	root := t.TempDir()
	names := make([]string, 0, len(frozen.Before))
	for name := range frozen.Before {
		names = append(names, name)
	}
	sort.Slice(names, func(i, j int) bool {
		left, right := strings.Count(names[i], "/"), strings.Count(names[j], "/")
		if left != right {
			return left < right
		}
		return names[i] < names[j]
	})
	for _, name := range names {
		entry := frozen.Before[name]
		path := filepath.Join(root, filepath.FromSlash(name))
		var err error
		if entry.Kind == "directory" {
			err = os.Mkdir(path, unixMode(entry.Mode))
		} else {
			err = os.WriteFile(path, entry.Bytes, unixMode(entry.Mode))
		}
		if err != nil {
			t.Fatal(err)
		}
		if err := os.Chmod(path, unixMode(entry.Mode)); err != nil {
			t.Fatal(err)
		}
	}
	plan, err := NewPlan(root, frozen.Operation, frozen.Watches, frozen.Before, frozen.After)
	if err != nil {
		t.Fatal(err)
	}
	assertSnapshot(t, root, plan.Before)
	return root, plan
}

func assertEveryDurableStageRecovers(t *testing.T, frozen Plan) {
	t.Helper()
	preview, err := frozen.Preview()
	if err != nil {
		t.Fatal(err)
	}
	for _, rollback := range []bool{false, true} {
		for stage := 0; stage <= len(preview); stage++ {
			t.Run(fmt.Sprintf("rollback=%t/stage=%d", rollback, stage), func(t *testing.T) {
				root, plan := fullConversionInput(t, frozen)
				reached := false
				_, err := (Engine{Checkpoint: func(name string) error {
					if name == fmt.Sprintf("apply:%d", stage) {
						reached = true
						return errComponentFault
					}
					return nil
				}}).Apply(root, plan)
				if !reached || !errors.Is(err, errComponentFault) {
					t.Fatalf("durable stage not reached: %v", err)
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
					t.Fatalf("phase %s want %s", journal.Phase, phase)
				}
				assertSnapshot(t, root, want)
				saved, err := os.ReadFile(filepath.Join(root, journalPath))
				if err != nil {
					t.Fatal(err)
				}
				fs := &faultFilesystem{root: root}
				if _, err := (Engine{Filesystem: fs}).Recover(root, rollback); err != nil {
					t.Fatal(err)
				}
				again, err := os.ReadFile(filepath.Join(root, journalPath))
				if err != nil {
					t.Fatal(err)
				}
				if !bytes.Equal(saved, again) || len(fs.trace) != 0 {
					t.Fatalf("terminal retry rewrote authors/journal: %v", fs.trace)
				}
				assertSnapshot(t, root, want)
			})
		}
	}
}

func TestFullConversionEveryDurableStageRecovers(t *testing.T) {
	for _, fixture := range fullConversionFixtures(t) {
		t.Run(fmt.Sprintf("subject=%s/root=%t", fixture.Subject, fixture.RootProject), func(t *testing.T) {
			assertEveryDurableStageRecovers(t, fixture.Plan)
		})
	}
}

func TestFullConversionPreparationFaultsRecoverOrRetry(t *testing.T) {
	for _, fixture := range fullConversionFixtures(t) {
		if fixture.Subject != "go" || fixture.RootProject {
			continue
		}
		t.Run("subject=go/project", func(t *testing.T) {
			for _, stage := range []string{"namespace", "directory", "backup", "publish", "partial"} {
				for _, rollback := range []bool{false, true} {
					t.Run(fmt.Sprintf("stage=%s/rollback=%t", stage, rollback), func(t *testing.T) {
						root, plan := fullConversionInput(t, fixture.Plan)
						engine := Engine{Checkpoint: func(name string) error {
							if stage != "partial" && name == "prepare:"+stage {
								return errComponentFault
							}
							return nil
						}}
						if stage == "partial" {
							fs := &faultFilesystem{root: root, fail: "write:" + preparing + "/journal.json.next", partialWrite: true}
							engine = Engine{Filesystem: fs}
						}
						if _, err := engine.Apply(root, plan); !errors.Is(err, errComponentFault) {
							t.Fatalf("preparation fault not reached: %v", err)
						}
						assertSnapshot(t, root, plan.Before)
						if stage == "namespace" {
							if _, err := (Engine{}).RecoverUnpublished(root, rollback); err != nil {
								t.Fatal(err)
							}
							if _, err := (Engine{}).Recover(root, rollback); err == nil {
								t.Fatal("namespace-only interruption unexpectedly had a journal")
							}
						} else {
							archived, err := (Engine{}).RecoverUnpublished(root, rollback)
							if err != nil {
								t.Fatal(err)
							}
							if archived {
								assertSnapshot(t, root, plan.Before)
							} else {
								journal, err := (Engine{}).Recover(root, rollback)
								if err != nil {
									t.Fatal(err)
								}
								want, phase := plan.After, "complete"
								if rollback {
									want, phase = plan.Before, "rolled_back"
								}
								if journal.Phase != phase {
									t.Fatalf("phase %q want %q", journal.Phase, phase)
								}
								assertSnapshot(t, root, want)
							}
						}
						if _, err := (Engine{}).Apply(root, plan); err != nil {
							t.Fatalf("retry original conversion: %v", err)
						}
						assertSnapshot(t, root, plan.After)
					})
				}
			}
		})
	}
}
