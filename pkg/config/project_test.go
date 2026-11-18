package config

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestProjectSelectionAmbiguityPathsAndLazyCreation(t *testing.T) {
	root := t.TempDir()
	root = canonical(root)
	cwd := filepath.Join(root, "work")
	for _, path := range []string{filepath.Join(root, "projects", "example"), filepath.Join(cwd, "example")} {
		if err := os.MkdirAll(path, 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(filepath.Join(path, "mind.yaml"), nil, 0o644); err != nil {
			t.Fatal(err)
		}
	}
	if err := os.WriteFile(filepath.Join(root, "minds.yaml"), []byte("projects: [example]\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	selector := "example"
	if _, err := ResolveProject(root, &selector, cwd, false); err == nil || !strings.Contains(err.Error(), "ambiguous") {
		t.Fatalf("ambiguous name selected: %v", err)
	}
	selector = "./example"
	if path, err := ResolveProject(root, &selector, cwd, false); err != nil || path != filepath.Join(cwd, "example") {
		t.Fatalf("cwd path: %s (%v)", path, err)
	}
	selector = "../projects/example"
	if _, err := ResolveProject(root, &selector, cwd, false); err == nil || !strings.Contains(err.Error(), "contains '..'") {
		t.Fatalf("traversal accepted: %v", err)
	}
	selector = "missing"
	if _, err := ResolveProject(root, &selector, cwd, false); err == nil || !strings.Contains(err.Error(), "not found") {
		t.Fatalf("missing project selected: %v", err)
	}
	if path, err := ResolveProject(root, &selector, cwd, true); err != nil || path != filepath.Join(root, "projects", "missing") {
		t.Fatalf("lazy creation candidate: %s (%v)", path, err)
	}
	if path, err := ResolveProject(root, nil, filepath.Join(cwd, "example"), false); err != nil || path != filepath.Join(cwd, "example") {
		t.Fatalf("current project: %s (%v)", path, err)
	}
	if _, err := os.Stat(filepath.Join(root, "projects", "missing")); !os.IsNotExist(err) {
		t.Fatal("resolving a new project created it")
	}
}
