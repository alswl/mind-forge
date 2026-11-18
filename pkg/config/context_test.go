package config

import (
	"os"
	"path/filepath"
	"testing"
)

func TestContextRootRequirementAndRepoFreeProject(t *testing.T) {
	cwd := t.TempDir()
	context, err := NewContext(ContextOptions{ResolveOptions: ResolveOptions{CWD: cwd, Config: "missing.toml"}, Project: "example", JSON: true, NoColor: true, Quiet: true})
	if err != nil {
		t.Fatal(err)
	}
	if context.Project() != "" || context.Format != "json" || context.Color || !context.Quiet || context.ConfigPath != "missing.toml" {
		t.Fatalf("context changed: %#v", context)
	}
	if _, err := context.RequireRepo(); err == nil {
		t.Fatal("missing root accepted")
	}
	if err := os.WriteFile(filepath.Join(cwd, "minds.yaml"), nil, 0o644); err != nil {
		t.Fatal(err)
	}
	context, err = NewContext(ContextOptions{ResolveOptions: ResolveOptions{CWD: cwd}, Project: "example"})
	if err != nil || context.Project() != "example" || context.Format != "text" || !context.Color {
		t.Fatalf("repo context: %#v (%v)", context, err)
	}
	if root, err := context.RequireRepo(); err != nil || root != canonical(cwd) {
		t.Fatalf("repo requirement: %s (%v)", root, err)
	}
	if _, err := NewContext(ContextOptions{ResolveOptions: ResolveOptions{CWD: cwd}, Quiet: true, Verbose: 1}); err == nil {
		t.Fatal("quiet/verbose conflict accepted")
	}
}

func TestPlatformConfigPathIgnoresRelativeXDGOverride(t *testing.T) {
	for _, item := range []struct{ platform, xdg, want string }{
		{"linux", "/settings", "/settings/mf/config.toml"},
		{"linux", "relative", "/home/example/.config/mf/config.toml"},
		{"darwin", "/settings", "/home/example/Library/Application Support/mf/config.toml"},
	} {
		got, err := DefaultConfigPath(item.platform, "/home/example", item.xdg)
		if err != nil || got != item.want {
			t.Fatalf("platform config path: %s (%v), want %s", got, err, item.want)
		}
	}
}
