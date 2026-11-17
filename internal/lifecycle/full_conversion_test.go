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

func TestFullConversionEveryDurableStageRecovers(t *testing.T) {
	for _, fixture := range fullConversionFixtures(t) {
		preview, err := fixture.Plan.Preview()
		if err != nil {
			t.Fatal(err)
		}
		for _, rollback := range []bool{false, true} {
			for stage := 0; stage <= len(preview); stage++ {
				t.Run(fmt.Sprintf("subject=%s/root=%t/rollback=%t/stage=%d", fixture.Subject, fixture.RootProject, rollback, stage), func(t *testing.T) {
					root, plan := fullConversionInput(t, fixture.Plan)
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
}
