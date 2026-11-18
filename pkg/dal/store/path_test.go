package store

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestPathBoundaryAndSelectorTraversal(t *testing.T) {
	root, outside := t.TempDir(), t.TempDir()
	root, _ = filepath.EvalSymlinks(root)
	if err := os.Mkdir(filepath.Join(root, "inside"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(outside, filepath.Join(root, "outside")); err != nil {
		t.Fatal(err)
	}
	for _, target := range []string{filepath.Join(root, "inside"), filepath.Join(root, "inside", "New Name"), filepath.Join(root, "missing", "nested")} {
		got, err := CanonicalWithin(root, target)
		if err != nil || got != target {
			t.Fatalf("safe path rejected: %s => %s (%v)", target, got, err)
		}
	}
	for _, target := range []string{outside, filepath.Join(root, "outside", "new"), filepath.Join(root, "outside", "missing", "nested")} {
		if _, err := CanonicalWithin(root, target); err == nil {
			t.Fatalf("escape accepted: %s", target)
		}
	}
	for _, input := range []string{"..", "a/../b", `a\..\b`} {
		if err := RejectTraversal(input, "repo"); err == nil || !strings.Contains(err.Error(), "contains '..'") {
			t.Fatalf("traversal accepted: %s (%v)", input, err)
		}
	}
	for _, input := range []string{".", "./a", "a..b", "..hidden"} {
		if err := RejectTraversal(input, "repo"); err != nil {
			t.Fatalf("literal dots rejected: %s (%v)", input, err)
		}
	}
}
