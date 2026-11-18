package content

import (
	"errors"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"github.com/alswl/mind-forge/internal/output"
)

func legacyBlockExists(t *testing.T, project, name string, want bool) {
	t.Helper()
	_, err := os.Stat(filepath.Join(project, "docs/my-article", name))
	if want && err != nil || !want && !os.IsNotExist(err) {
		t.Fatalf("exists(%s) = %v, want %v", name, err, want)
	}
}

func TestLegacyBlockRename(t *testing.T) {
	for _, name := range []string{"rename_block_by_filename", "rename_block_by_slug", "rename_block_not_directory_article", "rename_block_not_found", "rename_block_target_exists_without_force", "rename_block_target_exists_with_force", "rename_block_noop"} {
		t.Run(name, func(t *testing.T) {
			project := t.TempDir()
			article, selector, slug, oldName, newName := "docs/my-article", "02-notes.md", "thoughts", "02-notes.md", "02-thoughts.md"
			force, kind, fragment := false, "", ""
			putContent(t, project, article+"/01-opening.md", "# Title\n\nintro\n")
			body := "## Notes\n\nnotes body\n"
			switch name {
			case "rename_block_by_slug":
				selector, slug, oldName, newName, body = "refs", "references", "02-refs.md", "02-references.md", "## Refs\n\nrefs body\n"
			case "rename_block_not_directory_article":
				article, selector, slug, kind, fragment = "docs/my-article.md", "02-notes", "new-notes", "usage", "not a directory article"
				putContent(t, project, article, "# Title\n\ncontent\n")
			case "rename_block_not_found":
				selector, slug, kind, fragment = "03-missing", "new-slug", "not_found", "not found"
			case "rename_block_target_exists_without_force", "rename_block_target_exists_with_force":
				selector, slug, oldName, newName, body = "02-old.md", "existing", "02-old.md", "02-existing.md", "## Old\n\nold\n"
				putContent(t, project, "docs/my-article/01-opening.md", "# T\n\nintro\n")
				putContent(t, project, "docs/my-article/02-existing.md", "## Existing\n\ntext\n")
				force = name == "rename_block_target_exists_with_force"
				if !force {
					kind = "file_exists"
				}
			case "rename_block_noop":
				slug, newName, body = "notes", "02-notes.md", "## Notes\n\nbody\n"
			}
			if name != "rename_block_not_found" && name != "rename_block_not_directory_article" {
				putContent(t, project, article+"/"+oldName, body)
			}
			got, err := RenameBlock(project, article, selector, slug, force)
			if kind != "" {
				var failure *output.Error
				if !errors.As(err, &failure) || failure.Kind != kind || !strings.Contains(err.Error(), fragment) {
					t.Fatalf("rename = %+v, %v, want %s %s", got, err, kind, fragment)
				}
				return
			}
			if err != nil || got.OldFilename != oldName || got.NewFilename != newName {
				t.Fatalf("rename = %+v, %v", got, err)
			}
			legacyBlockExists(t, project, newName, true)
			if oldName != newName {
				legacyBlockExists(t, project, oldName, false)
			}
			raw, err := os.ReadFile(filepath.Join(project, article, newName))
			if err != nil || string(raw) != body {
				t.Fatalf("renamed body = %q, %v", raw, err)
			}
		})
	}
}

func TestLegacyBlockRemoval(t *testing.T) {
	for _, name := range []string{"remove_block_by_filename", "remove_block_by_slug", "remove_block_refuses_last_remaining_block", "remove_block_not_directory_article", "remove_block_not_found", "remove_block_dry_run_does_not_write", "remove_block_dry_run_still_refuses_last_remaining_block", "remove_block_ambiguous_slug"} {
		t.Run(name, func(t *testing.T) {
			project := t.TempDir()
			article, selector, selected := "docs/my-article", "02-notes.md", "02-notes.md"
			kind, fragment := "", ""
			dry := strings.Contains(name, "dry_run")
			putContent(t, project, article+"/01-opening.md", "# Title\n\nintro\n")
			switch name {
			case "remove_block_by_slug":
				selector, selected = "refs", "02-refs.md"
				putContent(t, project, article+"/02-refs.md", "## Refs\n\nrefs body\n")
			case "remove_block_refuses_last_remaining_block", "remove_block_dry_run_still_refuses_last_remaining_block":
				selector, kind = "01-opening.md", "usage"
				if !dry {
					fragment = "last remaining block"
				}
			case "remove_block_not_directory_article":
				article, selector, kind, fragment = "docs/my-article.md", "02-notes", "usage", "not a directory article"
				putContent(t, project, article, "# Title\n\ncontent\n")
			case "remove_block_not_found":
				selector, kind = "03-missing", "not_found"
				putContent(t, project, article+"/02-notes.md", "## Notes\n\nbody\n")
			case "remove_block_ambiguous_slug":
				if err := os.Remove(filepath.Join(project, article, "01-opening.md")); err != nil {
					t.Fatal(err)
				}
				putContent(t, project, article+"/01-notes.md", "# Notes A\n")
				putContent(t, project, article+"/02-notes.md", "# Notes B\n")
				putContent(t, project, article+"/03-third.md", "# Third\n")
				selector, kind, fragment = "notes", "usage", "multiple blocks match"
			default:
				putContent(t, project, article+"/02-notes.md", "## Notes\n\nnotes body\n")
			}
			got, err := RemoveBlock(project, article, selector, dry)
			if kind != "" {
				var failure *output.Error
				if !errors.As(err, &failure) || failure.Kind != kind || !strings.Contains(err.Error(), fragment) {
					t.Fatalf("remove = %+v, %v, want %s %s", got, err, kind, fragment)
				}
				if name == "remove_block_refuses_last_remaining_block" {
					legacyBlockExists(t, project, "01-opening.md", true)
				}
				return
			}
			if err != nil || got.RemovedFilename != selected || got.RemainingBlocks != 1 || got.DryRun != dry {
				t.Fatalf("remove = %+v, %v", got, err)
			}
			legacyBlockExists(t, project, selected, dry)
			legacyBlockExists(t, project, "01-opening.md", true)
		})
	}
}

func TestLegacyBlockReordering(t *testing.T) {
	t.Run("new_and_renumber_blocks_are_collision_safe", func(t *testing.T) {
		project := t.TempDir()
		putContent(t, project, "docs/article/01-first.md", "first\n")
		putContent(t, project, "docs/article/03-third.md", "third\n")
		created, err := NewBlock(project, "docs/article", "second", BlockEditOptions{After: ptrString("first"), Start: 1})
		want := []string{"01-first.md", "02-second.md", "03-third.md"}
		if err != nil || !reflect.DeepEqual([]string(created.Order), want) {
			t.Fatalf("create = %+v, %v", created, err)
		}
		if _, err := os.Stat(filepath.Join(project, "docs/article/02-second.md")); err != nil {
			t.Fatal(err)
		}
		got, err := RenumberBlocks(project, "docs/article", 1, false)
		if err != nil || !reflect.DeepEqual([]string(got.Order), want) {
			t.Fatalf("renumber = %+v, %v", got, err)
		}
	})
	t.Run("move_block_dry_run_preserves_tree", func(t *testing.T) {
		project := t.TempDir()
		putContent(t, project, "docs/article/01-first.md", "first\n")
		putContent(t, project, "docs/article/02-second.md", "second\n")
		got, err := MoveBlock(project, "docs/article", "first", BlockEditOptions{After: ptrString("second"), Start: 1, DryRun: true})
		if err != nil || !got.DryRun {
			t.Fatalf("move = %+v, %v", got, err)
		}
		for name, body := range map[string]string{"01-first.md": "first\n", "02-second.md": "second\n"} {
			raw, err := os.ReadFile(filepath.Join(project, "docs/article", name))
			if err != nil || string(raw) != body {
				t.Fatalf("preview changed %s: %q, %v", name, raw, err)
			}
		}
	})
	t.Run("move_block_reports_selected_block_when_bodies_match", func(t *testing.T) {
		project := t.TempDir()
		putContent(t, project, "docs/article/01-first.md", "same body\n")
		putContent(t, project, "docs/article/02-second.md", "same body\n")
		putContent(t, project, "docs/article/03-third.md", "other body\n")
		got, err := MoveBlock(project, "docs/article", "02-second", BlockEditOptions{After: ptrString("03-third"), Start: 1})
		if err != nil || got.NewPath == nil || *got.NewPath != "docs/article/03-second.md" {
			t.Fatalf("selected block = %+v, %v", got, err)
		}
	})
}

func TestRenumberBlocksClosesNumberGaps(t *testing.T) {
	project := t.TempDir()
	for name, body := range map[string]string{"01-first.md": "first\n", "09-second.md": "second\n", "20-third.md": "third\n"} {
		putContent(t, project, "docs/article/"+name, body)
	}
	got, err := RenumberBlocks(project, "docs/article", 1, false)
	if err != nil || !reflect.DeepEqual([]string(got.Order), []string{"01-first.md", "02-second.md", "03-third.md"}) {
		t.Fatalf("renumber gaps = %+v, %v", got, err)
	}
	for name, body := range map[string]string{"01-first.md": "first\n", "02-second.md": "second\n", "03-third.md": "third\n"} {
		raw, err := os.ReadFile(filepath.Join(project, "docs/article", name))
		if err != nil || string(raw) != body {
			t.Fatalf("renumber body(%s) = %q, %v", name, raw, err)
		}
	}
}
