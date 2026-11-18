package store

import (
	"os"
	"path/filepath"
	"testing"
)

func TestPendingRenameRestoresBothFilesAndOccupiedDestination(t *testing.T) {
	dir := t.TempDir()
	old, target := filepath.Join(dir, "old"), filepath.Join(dir, "target")
	for _, rollback := range []bool{true, false} {
		if err := os.WriteFile(old, []byte("author"), 0600); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(target, []byte("replaced"), 0640); err != nil {
			t.Fatal(err)
		}
		pending, err := BeginRename(old, target)
		if err != nil {
			t.Fatal(err)
		}
		if rollback {
			if err := pending.Rollback(); err != nil {
				t.Fatal(err)
			}
			for name, want := range map[string]string{old: "author", target: "replaced"} {
				raw, err := os.ReadFile(name)
				if err != nil || string(raw) != want {
					t.Fatalf("%s %q %v", name, raw, err)
				}
			}
		} else {
			if err := pending.Commit(); err != nil {
				t.Fatal(err)
			}
			if raw, err := os.ReadFile(target); err != nil || string(raw) != "author" {
				t.Fatal("move not committed")
			}
		}
	}
	if _, err := BeginRename(filepath.Join(dir, "missing"), target); err == nil {
		t.Fatal("missing source accepted")
	}
	if raw, err := os.ReadFile(target); err != nil || string(raw) != "author" {
		t.Fatal("failed move lost occupied target")
	}
}
