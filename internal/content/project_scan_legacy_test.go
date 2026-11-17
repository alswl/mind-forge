package content

import (
	"cmp"
	"os"
	"path/filepath"
	"reflect"
	"slices"
	"testing"
)

func TestLegacyProjectDirectoryScan(t *testing.T) {
	t.Run("test_scan_project_dirs_default_projects_dir", func(t *testing.T) {
		root := t.TempDir()
		putContent(t, root, "minds.yaml", "")
		putContent(t, root, "projects/project-a/mind.yaml", "")
		putContent(t, root, "projects/project-b/mind.yaml", "")
		if err := os.MkdirAll(filepath.Join(root, "projects/not-a-project"), 0o755); err != nil {
			t.Fatal(err)
		}
		putContent(t, root, "flat-project/mind.yaml", "")
		got, err := ScanProjects(root, "projects")
		if err != nil {
			t.Fatal(err)
		}
		slices.SortFunc(got, func(a, b ScannedProject) int { return cmp.Compare(a.Name, b.Name) })
		want := []ScannedProject{{Name: "project-a", Path: "./projects/project-a"}, {Name: "project-b", Path: "./projects/project-b"}}
		if !reflect.DeepEqual(got, want) {
			t.Fatalf("scanned projects = %#v, want %#v", got, want)
		}
	})
	t.Run("test_scan_project_dirs_flat_layout", func(t *testing.T) {
		root := t.TempDir()
		putContent(t, root, "minds.yaml", "")
		putContent(t, root, "flat-project/mind.yaml", "")
		got, err := ScanProjects(root, ".")
		if err != nil {
			t.Fatal(err)
		}
		want := []ScannedProject{{Name: "flat-project", Path: "./flat-project"}}
		if !reflect.DeepEqual(got, want) {
			t.Fatalf("scanned flat projects = %#v, want %#v", got, want)
		}
	})
}
