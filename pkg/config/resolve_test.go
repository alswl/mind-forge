package config

import (
	"os"
	"path/filepath"
	"testing"
)

func TestResolveRootUsesRootAndCWDNotRuntimeConfig(t *testing.T) {
	base := t.TempDir()
	for _, name := range []string{"cwd", "override", "configured"} {
		path := filepath.Join(base, name)
		if err := os.MkdirAll(path, 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(filepath.Join(path, "minds.yaml"), []byte("schema: '1'\n"), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	cwd := filepath.Join(base, "cwd")
	config := filepath.Join(base, "configured", "settings.yaml")
	if err := os.WriteFile(config, nil, 0o644); err != nil {
		t.Fatal(err)
	}
	for _, item := range []struct {
		options ResolveOptions
		want    string
	}{
		{ResolveOptions{CWD: cwd, Root: filepath.Join(base, "override"), Config: config}, filepath.Join(base, "override")},
		{ResolveOptions{CWD: cwd, Config: config}, cwd},
		{ResolveOptions{CWD: cwd}, cwd},
		{ResolveOptions{CWD: cwd, Config: filepath.Join(base, "missing")}, cwd},
	} {
		got, err := ResolveRoot(item.options)
		if err != nil {
			t.Fatal(err)
		}
		want := item.want
		if want != "" {
			want, err = filepath.EvalSymlinks(want)
			if err != nil {
				t.Fatal(err)
			}
		}
		if got != want {
			t.Fatalf("root=%q, want %q for %#v", got, want, item.options)
		}
	}
}

func TestExplicitInvalidRootDoesNotFallBack(t *testing.T) {
	if _, err := ResolveRoot(ResolveOptions{CWD: t.TempDir(), Root: "missing-root"}); err == nil {
		t.Fatal("explicit root must fail, not fall back")
	}
}

func TestDiscoveryRespectsDepthAndDoesNotCreateFiles(t *testing.T) {
	root := t.TempDir()
	if err := os.WriteFile(filepath.Join(root, "minds.yaml"), nil, 0o644); err != nil {
		t.Fatal(err)
	}
	child := filepath.Join(root, "a", "b")
	if err := os.MkdirAll(child, 0o755); err != nil {
		t.Fatal(err)
	}
	if got := DetectRoot(child, 1); got != "" {
		t.Fatalf("escaped maximum search depth: %s", got)
	}
	if got := DetectRoot(child, 2); got == "" {
		t.Fatal("root at allowed depth not discovered")
	}
	entries, err := os.ReadDir(child)
	if err != nil || len(entries) != 0 {
		t.Fatalf("discovery wrote files: %v (%v)", entries, err)
	}
}
