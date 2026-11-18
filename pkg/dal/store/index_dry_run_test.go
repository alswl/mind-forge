package store

import (
	"os"
	"path/filepath"
	"testing"
)

func TestLegacyIndexDryRunDoesNotCreateFile(t *testing.T) {
	root := t.TempDir()
	path := filepath.Join(root, "new.yaml")
	index, err := DecodeIndex([]byte("value: true\n"), path, false)
	if err != nil {
		t.Fatal(err)
	}
	if err := index.Write(path, index.Value, true); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(path); !os.IsNotExist(err) {
		t.Fatalf("dry-run created file: %v", err)
	}
	if entries, err := os.ReadDir(root); err != nil || len(entries) != 0 {
		t.Fatalf("dry-run created temporary entries: %v (%v)", entries, err)
	}
}
