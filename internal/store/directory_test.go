package store

import (
	"os"
	"path/filepath"
	"testing"
)

func TestDirectoryReplacementStagesBeforePublication(t *testing.T) {
	root := t.TempDir()
	path := filepath.Join(root, "article")
	if err := os.Mkdir(path, 0o750); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(path, "old.md"), []byte("old"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := ReplaceDirectory(path, map[string][]byte{"nested/bad.md": []byte("bad")}); err == nil {
		t.Fatal("accepted nonflat name")
	}
	if body, err := os.ReadFile(filepath.Join(path, "old.md")); err != nil || string(body) != "old" {
		t.Fatal("failed staging changed source", err)
	}
	if entries, _ := os.ReadDir(root); len(entries) != 1 {
		t.Fatal("staging leaked", entries)
	}
	if err := ReplaceDirectory(path, map[string][]byte{"new.md": []byte("new")}); err != nil {
		t.Fatal(err)
	}
	if body, err := os.ReadFile(filepath.Join(path, "new.md")); err != nil || string(body) != "new" {
		t.Fatal("publication", err)
	}
	if _, err := os.Stat(filepath.Join(path, "old.md")); !os.IsNotExist(err) {
		t.Fatal("old tree retained")
	}
	if entries, _ := os.ReadDir(root); len(entries) != 1 {
		t.Fatal("backup leaked", entries)
	}
	if info, err := os.Stat(path); err != nil || info.Mode().Perm() != 0o750 {
		t.Fatal("directory mode", info, err)
	}
}
