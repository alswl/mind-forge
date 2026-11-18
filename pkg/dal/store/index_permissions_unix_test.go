//go:build unix

package store

import (
	"errors"
	"os"
	"path/filepath"
	"testing"

	"github.com/alswl/mind-forge/pkg/common/model"
)

func TestIndexWriteReturnsPermissionErrorForReadOnlyParent(t *testing.T) {
	if os.Geteuid() == 0 {
		t.Skip("requires an unprivileged process to enforce directory permissions")
	}
	parent := t.TempDir()
	path := filepath.Join(parent, "mind-index.yaml")
	index, err := ReadIndex(path, false)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.Chmod(parent, 0o555); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if err := os.Chmod(parent, 0o755); err != nil {
			t.Error(err)
		}
	})
	err = index.Write(path, model.DefaultIndex(), false)
	var pathError *os.PathError
	if !errors.As(err, &pathError) || !errors.Is(err, os.ErrPermission) {
		t.Fatalf("write returned %v, want filesystem permission error", err)
	}
	entries, err := os.ReadDir(parent)
	if err != nil || len(entries) != 0 {
		t.Fatalf("failed write left files: %v, %v", entries, err)
	}
}
