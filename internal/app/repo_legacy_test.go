package app

import (
	"os"
	"path/filepath"
	"testing"
)

func TestLegacyRepoDetection(t *testing.T) {
	for _, name := range []string{"test_detect_repo_root_found", "test_detect_repo_root_not_found", "test_detect_repo_root_max_depth", "test_detect_repo_root_with_config_found", "test_detect_repo_root_with_config_not_found"} {
		t.Run(name, func(t *testing.T) {
			root := t.TempDir()
			if name == "test_detect_repo_root_found" || name == "test_detect_repo_root_with_config_found" {
				if err := os.WriteFile(filepath.Join(root, "minds.yaml"), nil, 0o644); err != nil {
					t.Fatal(err)
				}
			}
			if name == "test_detect_repo_root_found" {
				sub := filepath.Join(root, "a", "b")
				if err := os.MkdirAll(sub, 0o755); err != nil {
					t.Fatal(err)
				}
				if got := DetectRoot(sub, 50); got != canonical(root) {
					t.Fatalf("repo root = %q, want %q", got, canonical(root))
				}
				return
			}
			if name == "test_detect_repo_root_with_config_found" || name == "test_detect_repo_root_with_config_not_found" {
				config := filepath.Join(root, "mf.yaml")
				if err := os.WriteFile(config, nil, 0o644); err != nil {
					t.Fatal(err)
				}
				got, err := ResolveRoot(ResolveOptions{CWD: root, Config: config})
				if err != nil {
					t.Fatal(err)
				}
				want := ""
				if name == "test_detect_repo_root_with_config_found" {
					want = canonical(root)
				}
				if got != want {
					t.Fatalf("configured repo = %q, want %q", got, want)
				}
				return
			}
			depth := 50
			if name == "test_detect_repo_root_max_depth" {
				depth = 0
			}
			if got := DetectRoot(root, depth); got != "" {
				t.Fatalf("unexpected repo: %q", got)
			}
		})
	}
}

func TestRepoDepthLimitStopsBeforeKnownParent(t *testing.T) {
	root := t.TempDir()
	if err := os.WriteFile(filepath.Join(root, "minds.yaml"), nil, 0o644); err != nil {
		t.Fatal(err)
	}
	child := filepath.Join(root, "child")
	if err := os.Mkdir(child, 0o755); err != nil {
		t.Fatal(err)
	}
	if got := DetectRoot(child, 0); got != "" {
		t.Fatalf("zero-depth search reached parent: %q", got)
	}
	if got := DetectRoot(child, 1); got != canonical(root) {
		t.Fatalf("one-level search missed parent: %q", got)
	}
}

func TestLegacyContextOptions(t *testing.T) {
	root := t.TempDir()
	ctx, err := NewContext(ContextOptions{ResolveOptions: ResolveOptions{CWD: root, Config: filepath.Join(root, "config.toml")}, Project: "demo", Format: "json", NoColor: true, Quiet: true})
	if err != nil {
		t.Fatal(err)
	}
	if ctx.Format != "json" || !ctx.Quiet || ctx.Color || ctx.Project() != "" {
		t.Fatalf("context fields = %+v, project=%q", ctx, ctx.Project())
	}
}
