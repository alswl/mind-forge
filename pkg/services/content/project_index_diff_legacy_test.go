package content

import (
	"reflect"
	"testing"
	"time"

	"github.com/alswl/mind-forge/pkg/common/model"
)

func TestLegacyRepositoryIndexChanges(t *testing.T) {
	t.Run("test_compute_diff_add_remove", func(t *testing.T) {
		manifest := model.DefaultManifest()
		manifest.Projects = []model.ProjectEntry{{Name: "old-project", Path: "./old-project", CreatedAt: "2026-01-01T00:00:00Z"}}
		changes := compareProjects(manifest, []ScannedProject{{Name: "new-project", Path: "./new-project"}}, time.Date(2026, 4, 27, 0, 0, 0, 0, time.UTC))
		if len(changes.added) != 1 || changes.added[0].Name != "new-project" || len(changes.removed) != 1 || changes.removed[0].Name != "old-project" {
			t.Fatalf("index changes = %#v", changes)
		}
	})
	t.Run("test_reconcile_add_remove", func(t *testing.T) {
		keep := model.ProjectEntry{Name: "keep", Path: "./keep", CreatedAt: "2026-01-01T00:00:00Z"}
		remove := model.ProjectEntry{Name: "remove-me", Path: "./remove-me", CreatedAt: "2026-01-01T00:00:00Z"}
		added := model.ProjectEntry{Name: "new", Path: "./new", CreatedAt: "2026-04-27T00:00:00Z"}
		manifest := model.DefaultManifest()
		manifest.Projects = []model.ProjectEntry{keep, remove}
		got := reconcileProjects(manifest, projectIndexChanges{added: []model.ProjectEntry{added}, removed: []model.ProjectEntry{remove}})
		if !reflect.DeepEqual([]model.ProjectEntry(got.Projects), []model.ProjectEntry{keep, added}) {
			t.Fatalf("reconciled projects = %#v", got.Projects)
		}
	})
}

func TestProjectIndexChangesPreserveMetadataAndInput(t *testing.T) {
	archived := "2026-02-01T00:00:00Z"
	keep := model.ProjectEntry{Name: "keep", Path: "./old-path", CreatedAt: "original", ArchivedAt: &archived}
	manifest := model.DefaultManifest()
	manifest.Projects = []model.ProjectEntry{keep}
	stamp := time.Date(2026, 4, 27, 0, 0, 0, 0, time.UTC)
	changes := compareProjects(manifest, []ScannedProject{{Name: "z", Path: "./z"}, {Name: "keep", Path: "./new-path"}, {Name: "a", Path: "./a"}}, stamp)
	got := reconcileProjects(manifest, changes)
	if len(got.Projects) != 3 {
		t.Fatalf("reconciled projects = %#v", got.Projects)
	}
	wantKeep := keep
	wantKeep.Path = "./new-path"
	if got.Projects[0] != wantKeep || got.Projects[1].Name != "a" || got.Projects[2].Name != "z" || got.Projects[1].CreatedAt != "2026-04-27T00:00:00Z" {
		t.Fatalf("metadata/path/order changed: %#v", got.Projects)
	}
	if !reflect.DeepEqual([]model.ProjectEntry(manifest.Projects), []model.ProjectEntry{keep}) {
		t.Fatalf("input manifest mutated: %#v", manifest.Projects)
	}
}
