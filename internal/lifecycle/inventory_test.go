package lifecycle

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestSnapshotReadsBytesModesAndExcludesInternalState(t *testing.T) {
	root := t.TempDir()
	for _, directory := range []string{"project", ".git", ".mind-forge"} {
		if err := os.Mkdir(filepath.Join(root, directory), 0o755); err != nil {
			t.Fatal(err)
		}
	}
	if err := os.WriteFile(filepath.Join(root, "project", "asset"), []byte{0, 255, 3}, 0o640); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, ".git", "ignored"), nil, 0o600); err != nil {
		t.Fatal(err)
	}
	snapshot, err := SnapshotFiles(root, []string{"."})
	if err != nil {
		t.Fatal(err)
	}
	if len(snapshot) != 2 || snapshot["project"].Kind != "directory" || snapshot["project/asset"].Mode != 0o640 || !bytes.Equal(snapshot["project/asset"].Bytes, []byte{0, 255, 3}) {
		t.Fatalf("inventory: %#v", snapshot)
	}
	if entries, err := os.ReadDir(filepath.Join(root, ".mind-forge")); err != nil || len(entries) != 0 {
		t.Fatal("inspection created transaction state")
	}
	filtered, err := SnapshotFiles(root, []string{"project/asset", "missing"})
	if err != nil || len(filtered) != 1 {
		t.Fatalf("filtered inventory: %#v (%v)", filtered, err)
	}
}

func TestSnapshotRejectsSymlinksHardlinksAndUnsafeNames(t *testing.T) {
	for _, kind := range []string{"symlink", "hardlink"} {
		t.Run(kind, func(t *testing.T) {
			root := t.TempDir()
			target := filepath.Join(t.TempDir(), "target")
			if err := os.WriteFile(target, []byte("payload"), 0o644); err != nil {
				t.Fatal(err)
			}
			var err error
			switch kind {
			case "symlink":
				err = os.Symlink(target, filepath.Join(root, "entry"))
			case "hardlink":
				err = os.Link(target, filepath.Join(root, "entry"))
			}
			if err != nil {
				t.Fatal(err)
			}
			if _, err := SnapshotFiles(root, []string{"."}); err == nil {
				t.Fatalf("unsafe inventory accepted: %s", kind)
			}
		})
	}
	if err := relativePath(string([]byte{'x', 255})); err == nil {
		t.Fatal("non UTF-8 inventory path accepted")
	}
	if _, err := SnapshotFiles(t.TempDir(), []string{"../outside"}); err == nil || !strings.Contains(err.Error(), "outside") {
		t.Fatalf("unsafe watch accepted: %v", err)
	}
}
