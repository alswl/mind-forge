package store

import (
	"os"
	"testing"
)

func TestDirectoryInodeLockExcludesAnotherDescriptorWithoutCreatingFiles(t *testing.T) {
	root := t.TempDir()
	first, err := AcquireWriteLock(root)
	if err != nil {
		t.Fatal(err)
	}
	defer first.Close()
	if second, err := AcquireWriteLock(root); err == nil {
		second.Close()
		t.Fatal("second descriptor acquired the lock")
	}
	if err := first.Close(); err != nil {
		t.Fatal(err)
	}
	third, err := AcquireWriteLock(root)
	if err != nil {
		t.Fatalf("lock not released: %v", err)
	}
	defer third.Close()
	entries, err := os.ReadDir(root)
	if err != nil || len(entries) != 0 {
		t.Fatalf("directory lock created state: %v (%v)", entries, err)
	}
}
