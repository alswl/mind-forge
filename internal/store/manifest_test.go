package store

import (
	"os"
	"path/filepath"
	"testing"
)

func TestManifestLegacyPathsAndCanonicalWrite(t *testing.T) {
	path := filepath.Join(t.TempDir(), "minds.yaml")
	raw := "schema: '1'\nprojects_dir: /workspace/\nprojects:\n- example\n- ./nested/other\n- name: modern\n  path: ./custom/modern\n  created_at: '2026-01-01'\n"
	if err := os.WriteFile(path, []byte(raw), 0o600); err != nil {
		t.Fatal(err)
	}
	manifest, err := ReadManifest(path)
	if err != nil {
		t.Fatal(err)
	}
	want := []string{"./workspace/example", "./nested/other", "./custom/modern"}
	for i, entry := range manifest.Projects {
		if entry.Path != want[i] {
			t.Fatalf("entry %d: %#v", i, entry)
		}
	}
	if actual, _ := os.ReadFile(path); string(actual) != raw {
		t.Fatal("manifest read changed author bytes")
	}
	if err := WriteManifest(path, manifest); err != nil {
		t.Fatal(err)
	}
	expected := "schema: '1'\nprojects_dir: /workspace/\nprojects:\n- workspace/example\n- nested/other\n- custom/modern\n"
	if actual, _ := os.ReadFile(path); string(actual) != expected {
		t.Fatalf("canonical manifest: %s", actual)
	}
	info, _ := os.Stat(path)
	if info.Mode().Perm() != 0o600 {
		t.Fatal("manifest permissions changed")
	}
}

func TestFlatManifestRetainsDotPrefix(t *testing.T) {
	path := filepath.Join(t.TempDir(), "minds.yaml")
	if err := os.WriteFile(path, []byte("projects_dir: .\nprojects: [example]\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	manifest, err := ReadManifest(path)
	if err != nil {
		t.Fatal(err)
	}
	if manifest.Projects[0].Path != "./example" {
		t.Fatal(manifest.Projects)
	}
	if err := WriteManifest(path, manifest); err != nil {
		t.Fatal(err)
	}
	loaded, err := ReadManifest(path)
	if err != nil || loaded.Projects[0].Path != "./example" {
		t.Fatalf("flat project drift: %#v (%v)", loaded, err)
	}
}
