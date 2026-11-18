package content

import (
	"bytes"
	"os"
	"path/filepath"
	"reflect"
	"testing"
)

func TestContentIndexChecksAndReconcilesOnlyProjections(t *testing.T) {
	root := mindProject(t, "category")
	if _, err := CreateMind(root, root, "idea", MindCreateOptions{}); err != nil {
		t.Fatal(err)
	}
	if _, err := CreateOutput(root, root, "idea", "draft", OutputCreateOptions{File: true}); err != nil {
		t.Fatal(err)
	}
	indexPath := filepath.Join(root, "mind-index.yaml")
	raw := []byte("# index header\nschema: '1'\nsources: []\nopaque: {literal: [1, 2]} # keep exactly\n")
	if err := os.WriteFile(indexPath, raw, 0o640); err != nil {
		t.Fatal(err)
	}
	first, err := InspectContent(root, root)
	if err != nil || first["consistent"] != false {
		t.Fatalf("missing projection: %#v (%v)", first, err)
	}
	second, err := InspectContent(root, root)
	if err != nil || !reflect.DeepEqual(first, second) {
		t.Fatal("check was not deterministic")
	}
	if _, err := ReconcileContent(root, root, true); err != nil {
		t.Fatal(err)
	}
	if current, _ := os.ReadFile(indexPath); !bytes.Equal(current, raw) {
		t.Fatal("preview rewrote index")
	}
	author := filepath.Join(root, "outputs/idea/draft.md")
	before, _ := os.ReadFile(author)
	if _, err := ReconcileContent(root, root, false); err != nil {
		t.Fatal(err)
	}
	current, _ := os.ReadFile(indexPath)
	if !bytes.Contains(current, []byte("opaque: {literal: [1, 2]} # keep exactly")) || !bytes.Contains(current, []byte("sources: []")) {
		t.Fatalf("unrelated index text changed: %s", current)
	}
	if after, _ := os.ReadFile(author); !bytes.Equal(before, after) {
		t.Fatal("index changed author content")
	}
	checked, err := InspectContent(root, root)
	if err != nil || checked["consistent"] != true {
		t.Fatalf("reconcile left projection differences: %#v (%v)", checked, err)
	}
	if err := os.Rename(author, filepath.Join(root, "outputs/idea/renamed.md")); err != nil {
		t.Fatal(err)
	}
	checked, err = InspectContent(root, root)
	if err != nil || checked["consistent"] != false {
		t.Fatal("stale projection was accepted")
	}
}

func TestContentCheckKeepsAuthorityAndSkipsToolCacheContents(t *testing.T) {
	root := mindProject(t, "bundle")
	if _, err := CreateMind(root, root, "idea", MindCreateOptions{}); err != nil {
		t.Fatal(err)
	}
	putContent(t, root, "idea/sources/material.md", "# Material\n")
	putContent(t, root, "idea/reviews/data.json", "{\"evidence\":true}\n")
	if err := os.MkdirAll(filepath.Join(root, "cache"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(t.TempDir(), filepath.Join(root, "cache/unrelated")); err != nil {
		t.Fatal(err)
	}
	raw := "schema: '1'\nsources:\n- name: material\n  path: idea/sources/material.md\n  mind: other\n"
	putContent(t, root, "mind-index.yaml", raw)
	report, err := InspectContent(root, root)
	if err != nil || report["consistent"] != false {
		t.Fatalf("binding check: %#v (%v)", report, err)
	}
	if _, err := ReconcileContent(root, root, false); err != nil {
		t.Fatal(err)
	}
	updated, _ := os.ReadFile(filepath.Join(root, "mind-index.yaml"))
	if !bytes.Contains(updated, []byte("  mind: other")) {
		t.Fatalf("reconcile edited an authoritative binding: %s", updated)
	}
	items, err := DiscoverAdditional(root, root)
	if err != nil || len(items) != 1 || items[0].Identity != "::idea::additional::reviews" {
		t.Fatalf("additional structure: %#v (%v)", items, err)
	}
}
