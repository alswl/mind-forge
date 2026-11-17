package content

import (
	"bytes"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/alswl/mind-forge/internal/store"
)

func TestProjectIndexPreviewsAndReconcilesOnlyAbsentDocs(t *testing.T) {
	root := canonicalPath(t.TempDir())
	putContent(t, root, "minds.yaml", "schema: '1'\nprojects_dir: projects\nprojects: [old, missing]\n")
	project := filepath.Join(root, "projects/old")
	putContent(t, project, "mind.yaml", "schema: '1'\n")
	putContent(t, project, "docs/new.md", "# New\n")
	putContent(t, project, "external/keep.md", "# Existing outside docs\n")
	putContent(t, project, "mind-index.yaml", "schema: '1'\narticles:\n- title: Gone\n  type: blog\n  status: draft\n  article_path: docs/gone.md\n- title: Existing\n  type: blog\n  status: draft\n  article_path: external/keep.md\n  created_at: original\nterms: [] # retain exact\ncustom: {note: retained}\n")
	putContent(t, root, "projects/discovered/mind.yaml", "schema: '1'\n")
	before, _ := os.ReadFile(filepath.Join(project, "mind-index.yaml"))
	manifest, _ := os.ReadFile(filepath.Join(root, "minds.yaml"))
	preview, warnings, err := IndexProjects(root, true, time.Time{})
	if err != nil || len(warnings) != 0 || !preview.DryRun {
		t.Fatalf("%#v %v %v", preview, warnings, err)
	}
	after, _ := os.ReadFile(filepath.Join(project, "mind-index.yaml"))
	current, _ := os.ReadFile(filepath.Join(root, "minds.yaml"))
	if !bytes.Equal(before, after) || !bytes.Equal(manifest, current) {
		t.Fatal("project index preview wrote")
	}
	report, warnings, err := IndexProjects(root, false, time.Time{})
	if err != nil || len(warnings) != 0 || report.Kind != "project" {
		t.Fatalf("%#v %v %v", report, warnings, err)
	}
	doc, err := store.ReadIndex(filepath.Join(project, "mind-index.yaml"), false)
	if err != nil {
		t.Fatal(err)
	}
	if doc.Value.Articles == nil || len(*doc.Value.Articles) != 2 || (*doc.Value.Articles)[0].Path != "external/keep.md" || (*doc.Value.Articles)[0].CreatedAt != "original" {
		t.Fatalf("%#v", doc.Value.Articles)
	}
	after, _ = os.ReadFile(filepath.Join(project, "mind-index.yaml"))
	if !bytes.Contains(after, []byte("terms: [] # retain exact\ncustom: {note: retained}\n")) {
		t.Fatal("unrelated sections changed")
	}
	m, err := store.ReadManifest(filepath.Join(root, "minds.yaml"))
	if err != nil || len(m.Projects) != 2 || m.Projects[0].Name != "old" || m.Projects[1].Name != "discovered" {
		t.Fatalf("%#v %v", m, err)
	}
}
