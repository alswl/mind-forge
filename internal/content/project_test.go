package content

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestProjectRenameUsesFilesystemForUntrackedContent(t *testing.T) {
	root := canonicalPath(t.TempDir())
	putContent(t, root, "minds.yaml", "schema: '1'\nprojects_dir: projects\nprojects: [alpha]\n")
	putContent(t, root, "projects/alpha/mind.yaml", "schema: '1'\n")
	putContent(t, root, "projects/alpha/docs/keep.md", "# Keep\n")
	var calls []string
	ops := ProjectOperations{Git: func(_ context.Context, _ string, args ...string) ([]byte, []byte, error) {
		calls = append(calls, strings.Join(args, " "))
		return nil, nil, nil
	}}
	report, err := ops.RenameProject(context.Background(), root, "alpha", "beta")
	if err != nil || report.To != "projects/beta" || len(calls) != 2 || calls[1] != "ls-files projects/alpha" {
		t.Fatalf("untracked rename: %#v %#v (%v)", report, calls, err)
	}
	if got, err := os.ReadFile(filepath.Join(root, "projects/beta/docs/keep.md")); err != nil || string(got) != "# Keep\n" {
		t.Fatal("rename lost authored content")
	}
	if _, err := ops.RenameProject(context.Background(), root, "beta", "projects/gamma"); err == nil {
		t.Fatal("rename accepted a path as the new name")
	}
}

func TestProjectArchiveUsesGitMoveAndRemovePreviewIsReadOnly(t *testing.T) {
	root := canonicalPath(t.TempDir())
	putContent(t, root, "minds.yaml", "schema: '1'\nprojects_dir: projects\nprojects: [alpha]\n")
	putContent(t, root, "projects/alpha/mind.yaml", "schema: '1'\n")
	manifest, _ := os.ReadFile(filepath.Join(root, "minds.yaml"))
	path, err := RemoveProject(root, "alpha", true)
	if err != nil || path != filepath.Join(root, "projects/alpha") {
		t.Fatalf("remove preview: %s (%v)", path, err)
	}
	if current, _ := os.ReadFile(filepath.Join(root, "minds.yaml")); string(current) != string(manifest) {
		t.Fatal("remove preview rewrote manifest")
	}
	var calls []string
	ops := ProjectOperations{Git: func(_ context.Context, dir string, args ...string) ([]byte, []byte, error) {
		calls = append(calls, strings.Join(args, " "))
		if args[0] == "mv" {
			return nil, nil, os.Rename(filepath.Join(dir, args[1]), filepath.Join(dir, args[2]))
		}
		return nil, nil, nil
	}}
	report, err := ops.ArchiveProject(context.Background(), root, "alpha")
	if err != nil || report.To != "_archived/alpha" || len(calls) != 2 || calls[1] != "mv projects/alpha _archived/alpha" {
		t.Fatalf("archive: %#v %#v (%v)", report, calls, err)
	}
	if _, err := os.Stat(filepath.Join(root, "_archived/alpha/mind.yaml")); err != nil {
		t.Fatal(err)
	}
	if _, err := RemoveProject(root, "../outside", false); err == nil {
		t.Fatal("traversal removal accepted")
	}
}

func TestProjectImportCountsExistingSourcesAndCreatesWelcomeIndex(t *testing.T) {
	root := canonicalPath(t.TempDir())
	if _, err := InitializeRepository(root); err != nil {
		t.Fatal(err)
	}
	putContent(t, root, "projects/imported/sources/first.md", "# One\n")
	putContent(t, root, "projects/imported/sources/nested/second.txt", "Two\n")
	report, err := ImportProject(root, filepath.Join(root, "projects/imported"), ProjectImportOptions{Now: time.Date(2026, 1, 1, 0, 0, 0, 0, time.UTC)})
	if err != nil || report.ArticleCount != 2 || !report.Scaffolded {
		t.Fatalf("import: %#v (%v)", report, err)
	}
	index := readProjectIndex(report.Path)
	if index.Articles == nil || len(*index.Articles) != 1 || (*index.Articles)[0].Title != "Welcome" {
		t.Fatal("legacy import index changed")
	}
	if _, err := ImportProject(root, report.Path, ProjectImportOptions{}); err == nil {
		t.Fatal("import overwrote mind.yaml without force")
	}
}

func TestCreateProjectPreviewAndScaffold(t *testing.T) {
	root, err := filepath.EvalSymlinks(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	if _, err := InitializeRepository(root); err != nil {
		t.Fatal(err)
	}
	layout := "bundle"
	clock := time.Date(2026, 1, 2, 3, 4, 5, 0, time.UTC)
	preview, err := CreateProject(root, root, "alpha", ProjectCreateOptions{Layout: &layout, DryRun: true, Now: clock})
	if err != nil || preview.Identity != "projects/alpha" || !preview.DryRun {
		t.Fatalf("preview: %#v (%v)", preview, err)
	}
	if _, err := os.Stat(filepath.Join(root, "projects/alpha")); !os.IsNotExist(err) {
		t.Fatal("preview created content")
	}
	created, err := CreateProject(root, root, "alpha", ProjectCreateOptions{Layout: &layout, Now: clock})
	if err != nil || created.Details["created_at"] != "2026-01-02T03:04:05Z" {
		t.Fatalf("create: %#v (%v)", created, err)
	}
	config, err := ConfigFor(filepath.Join(root, "projects/alpha"))
	if err != nil || config.ContentLayout == nil || *config.ContentLayout != "bundle" {
		t.Fatalf("layout: %#v (%v)", config, err)
	}
	for _, path := range []string{"docs", "sources", "assets", "mind.yaml", "mind-index.yaml"} {
		if _, err := os.Stat(filepath.Join(root, "projects/alpha", path)); err != nil {
			t.Fatal(err)
		}
	}
	if _, err := CreateProject(root, root, "alpha", ProjectCreateOptions{}); err == nil {
		t.Fatal("nonempty existing project overwritten")
	}
	if _, err := CreateProject(root, filepath.Join(root, "projects/alpha"), "nested", ProjectCreateOptions{}); err == nil {
		t.Fatal("nested project created")
	}
	listed, _, err := ListProjects(root)
	if err != nil || len(listed) != 1 || listed[0].Name != "alpha" || listed[0].Path != "./projects/alpha" {
		t.Fatalf("list: %#v (%v)", listed, err)
	}
}

func TestProjectDescriptionUpdatePreservesUnrelatedConfig(t *testing.T) {
	root := t.TempDir()
	putContent(t, root, "minds.yaml", "schema: '1'\nprojects_dir: projects\nprojects: [alpha]\n")
	raw := "# header\nschema: '1'\nproject:\n  name: alpha\n  description: before\n  extra: retained\nopaque: {literal: [1, 2]} # keep exactly\n"
	putContent(t, root, "projects/alpha/mind.yaml", raw)
	description := "after"
	if _, err := UpdateProject(root, "alpha", &description, false, true); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(root, "projects/alpha/mind.yaml")
	if got, _ := os.ReadFile(path); string(got) != raw {
		t.Fatal("update preview rewrote config")
	}
	result, err := UpdateProject(root, "alpha", &description, false, false)
	if err != nil || result.Identity != "alpha" {
		t.Fatalf("update: %#v (%v)", result, err)
	}
	got, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(got), "opaque: {literal: [1, 2]} # keep exactly") || !strings.Contains(string(got), "extra: retained") {
		t.Fatalf("update lost unrelated config: %s", got)
	}
	config, err := ConfigFor(filepath.Dir(path))
	if err != nil || config.Project.Description == nil || *config.Project.Description != "after" {
		t.Fatalf("description: %#v (%v)", config, err)
	}
	if _, err := UpdateProject(root, "alpha", nil, false, false); err == nil {
		t.Fatal("empty update accepted")
	}
	if _, err := UpdateProject(root, "alpha", &description, true, false); err == nil {
		t.Fatal("conflicting update accepted")
	}
}

func TestProjectMutationsRefuseOutsideAndRepositoryRoot(t *testing.T) {
	parent := canonicalPath(t.TempDir())
	root := filepath.Join(parent, "repo")
	putContent(t, root, "minds.yaml", "schema: '1'\nprojects_dir: '.'\nprojects: []\n")
	putContent(t, root, "mind.yaml", "schema: '1'\n")
	putContent(t, parent, "outside/mind.yaml", "schema: '1'\nproject: {name: outside, description: keep}\n")
	description := "changed"
	if _, err := UpdateProject(root, "../outside", &description, false, false); err == nil {
		t.Fatal("update escaped the repo boundary")
	}
	if _, err := RemoveProject(root, ".", false); err == nil {
		t.Fatal("removal accepted the repository root")
	}
	if raw, err := os.ReadFile(filepath.Join(parent, "outside/mind.yaml")); err != nil || !strings.Contains(string(raw), "description: keep") {
		t.Fatal("outside configuration changed")
	}
	if _, err := os.Stat(filepath.Join(root, "minds.yaml")); err != nil {
		t.Fatal("repository manifest removed")
	}
}
