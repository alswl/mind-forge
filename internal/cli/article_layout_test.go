package cli

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/alswl/mind-forge/internal/app"
	"github.com/alswl/mind-forge/internal/output"
)

func TestArticleLayoutGuardPreservesFilesAndRejectsModernProjects(t *testing.T) {
	for _, layout := range []string{"category", "bundle", "legacy", "malformed"} {
		for _, explicit := range []bool{false, true} {
			name := layout + "/automatic"
			if explicit {
				name = layout + "/explicit"
			}
			t.Run(name, func(t *testing.T) {
				root := t.TempDir()
				project := filepath.Join(root, "lab")
				if err := os.Mkdir(project, 0o755); err != nil {
					t.Fatal(err)
				}
				if err := os.WriteFile(filepath.Join(root, "minds.yaml"), []byte("schema: '1'\nprojects_dir: '.'\nprojects: [lab]\n"), 0o644); err != nil {
					t.Fatal(err)
				}
				body := "schema: '1'\n"
				if layout == "category" || layout == "bundle" {
					body += "content_layout: " + layout + "\n"
				} else if layout == "malformed" {
					body = "schema: [unfinished\n"
				}
				config := filepath.Join(project, "mind.yaml")
				if err := os.WriteFile(config, []byte(body), 0o640); err != nil {
					t.Fatal(err)
				}
				before, err := os.Stat(config)
				if err != nil {
					t.Fatal(err)
				}
				var selector *string
				if explicit {
					value := "lab"
					selector = &value
				}
				err = checkArticleLayout(&app.Context{RepoRoot: root, CWD: project}, selector)
				if layout == "legacy" {
					if err != nil {
						t.Fatal(err)
					}
				} else {
					var failure *output.Error
					kind := "usage"
					if layout == "malformed" {
						kind = "parse_error"
					}
					if !errors.As(err, &failure) || failure.Kind != kind {
						t.Fatalf("layout %s returned %v, want %s", layout, err, kind)
					}
					if kind == "usage" && (!strings.Contains(failure.Message, "mf mind") || !strings.Contains(failure.Message, "mf output")) {
						t.Fatalf("layout refusal lacks supported commands: %v", failure)
					}
				}
				after, err := os.Stat(config)
				if err != nil {
					t.Fatal(err)
				}
				raw, err := os.ReadFile(config)
				if err != nil || string(raw) != body || before.Mode() != after.Mode() || !before.ModTime().Equal(after.ModTime()) {
					t.Fatalf("layout guard changed config: %v", err)
				}
				entries, err := os.ReadDir(project)
				if err != nil || len(entries) != 1 {
					t.Fatalf("layout guard created files: %v, %v", entries, err)
				}
			})
		}
	}
}

func TestArticleLayoutGuardLeavesUnresolvedProjectToCommand(t *testing.T) {
	root := t.TempDir()
	missing := "missing"
	if err := checkArticleLayout(&app.Context{RepoRoot: root, CWD: root}, &missing); err != nil {
		t.Fatalf("unresolved project must retain the command's own error: %v", err)
	}
}
