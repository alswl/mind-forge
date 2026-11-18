package content

import (
	"os"
	"path/filepath"
	"testing"
	"time"
)

func mindProject(t *testing.T, layout string) string {
	t.Helper()
	root := canonicalPath(t.TempDir())
	putContent(t, root, "minds.yaml", "schema: '1'\nprojects_dir: '.'\nprojects: ['.']\n")
	putContent(t, root, "mind.yaml", "# keep header\nschema: '1'\ncontent_layout: "+layout+"\nopaque: {keep: [1, 2]} # literal\n")
	putContent(t, root, "mind-index.yaml", "schema: '1'\n")
	return root
}

func TestMindRegistrationCreatesNoRolesOrBundleDirectory(t *testing.T) {
	for _, layout := range []string{"category", "bundle"} {
		root := mindProject(t, layout)
		clock := time.Date(2026, 1, 1, 0, 0, 0, 0, time.UTC)
		before, _ := os.ReadFile(filepath.Join(root, "mind.yaml"))
		preview, err := CreateMind(root, root, "idea", MindCreateOptions{DryRun: true, Now: clock})
		if err != nil || preview["path"] != nil || preview["dry_run"] != true {
			t.Fatalf("preview: %#v (%v)", preview, err)
		}
		if after, _ := os.ReadFile(filepath.Join(root, "mind.yaml")); string(before) != string(after) {
			t.Fatal("registration preview wrote config")
		}
		created, err := CreateMind(root, root, "idea", MindCreateOptions{Now: clock})
		if err != nil || created["identity"] != "::idea" || created["path"] != nil {
			t.Fatalf("registration: %#v (%v)", created, err)
		}
		if _, err := os.Stat(filepath.Join(root, "idea")); !os.IsNotExist(err) {
			t.Fatal("registration invented a bundle directory")
		}
		minds, err := Discover(root, root)
		if err != nil || len(minds) != 1 || minds[0].Path != nil || len(minds[0].Outputs) != 0 || len(minds[0].Roles) != 0 {
			t.Fatalf("zero-output mind: %#v (%v)", minds, err)
		}
		if _, err := CreateMind(root, root, "IDEA", MindCreateOptions{}); err == nil {
			t.Fatal("ASCII-case duplicate registration accepted")
		}
		if _, err := CreateMind(root, root, "cache", MindCreateOptions{}); err == nil {
			t.Fatal("reserved mind accepted")
		}
	}
}

func TestOutputCreationPreservesShapeAndRefusesCollisions(t *testing.T) {
	for _, layout := range []string{"category", "bundle"} {
		root := mindProject(t, layout)
		if _, err := CreateMind(root, root, "idea", MindCreateOptions{}); err != nil {
			t.Fatal(err)
		}
		preview, err := CreateOutput(root, root, "idea", "draft", OutputCreateOptions{DryRun: true})
		if err != nil || preview["path"] != nil {
			t.Fatalf("output preview: %#v (%v)", preview, err)
		}
		path := OutputRoot(layout, "idea") + "/draft"
		if _, err := os.Stat(filepath.Join(root, path)); !os.IsNotExist(err) {
			t.Fatal("preview created output")
		}
		created, err := CreateOutput(root, root, "idea", "draft", OutputCreateOptions{})
		if err != nil || created["identity"] != "::idea::draft" || created["path"] != path {
			t.Fatalf("directory output: %#v (%v)", created, err)
		}
		if got, err := os.ReadFile(filepath.Join(root, path, "01-opening.md")); err != nil || string(got) != "# draft\n\n" {
			t.Fatalf("opening content: %q (%v)", got, err)
		}
		if _, err := CreateOutput(root, root, "idea", "draft", OutputCreateOptions{File: true}); err == nil {
			t.Fatal("same-name alternate shape accepted")
		}
		if _, err := CreateOutput(root, root, "idea", "final", OutputCreateOptions{File: true}); err != nil {
			t.Fatal(err)
		}
		resolved, err := ResolveOutput(root, root, "idea::final")
		if err != nil || resolved.Shape != "file" {
			t.Fatalf("file output: %#v (%v)", resolved, err)
		}
	}
}
