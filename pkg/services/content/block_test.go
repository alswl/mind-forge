package content

import (
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
)

func TestBlockSelectorFormsAndAmbiguity(t *testing.T) {
	files := []string{"docs/draft/01-opening.md", "docs/draft/02-notes.md", "docs/draft/03-notes.md"}
	for _, tc := range []struct{ selector, want string }{{"01-opening.md", "01-opening.md"}, {"01-opening", "01-opening.md"}, {"opening", "01-opening.md"}, {"docs/draft/01-opening.md", "01-opening.md"}, {`docs\draft\01-opening.md`, "01-opening.md"}} {
		got, err := ResolveBlockFilename(files, "docs/draft", tc.selector)
		if err != nil || got != tc.want {
			t.Errorf("%q: %q (%v)", tc.selector, got, err)
		}
	}
	for _, selector := range []string{"notes", "missing.md", "docs/other/01-opening.md", "docs/draft/../01-opening.md", "notes.txt"} {
		if _, err := ResolveBlockFilename(files, "docs/draft", selector); err == nil {
			t.Errorf("accepted %q", selector)
		}
	}
}

func TestBlockEditsKeepBodiesAndPreviewReadOnly(t *testing.T) {
	root := canonicalPath(t.TempDir())
	putContent(t, root, "mind.yaml", "schema: '1'\nplugins:\n  typora-front-matter:\n    enabled: false\n")
	putContent(t, root, "docs/draft/01-opening.md", "# Title\n")
	putContent(t, root, "docs/draft/05-notes.md", "## Notes\r\nkeep\r\n")
	preview, err := NewBlock(root, "docs/draft", "Middle", BlockEditOptions{After: ptrString("opening"), Start: 7, DryRun: true})
	if err != nil || !reflect.DeepEqual([]string(preview.Order), []string{"07-opening.md", "08-middle.md", "09-notes.md"}) {
		t.Fatalf("preview %#v (%v)", preview, err)
	}
	if _, err := os.Stat(filepath.Join(root, "docs/draft/01-opening.md")); err != nil {
		t.Fatal("preview changed source", err)
	}
	created, err := NewBlock(root, "docs/draft", "Middle", BlockEditOptions{After: ptrString("opening"), Start: 7})
	if err != nil || created.NewPath == nil || *created.NewPath != "docs/draft/08-middle.md" {
		t.Fatalf("new %#v (%v)", created, err)
	}
	if got, _ := os.ReadFile(filepath.Join(root, "docs/draft/09-notes.md")); string(got) != "## Notes\r\nkeep\r\n" {
		t.Fatalf("body changed %q", got)
	}
	moved, err := MoveBlock(root, "docs/draft", "opening", BlockEditOptions{After: ptrString("notes"), Start: 1})
	if err != nil || !reflect.DeepEqual([]string(moved.Order), []string{"01-middle.md", "02-notes.md", "03-opening.md"}) {
		t.Fatalf("move %#v (%v)", moved, err)
	}
	if _, err := MoveBlock(root, "docs/draft", "notes", BlockEditOptions{After: ptrString("notes"), Start: 1}); err == nil {
		t.Fatal("accepted removed anchor")
	}
	if _, err := NewBlock(root, "docs/draft", "!!!", BlockEditOptions{Start: 1}); err == nil {
		t.Fatal("accepted empty slug")
	}
	if _, err := NewBlock(root, "docs/draft", "notes", BlockEditOptions{Start: 1}); err == nil {
		t.Fatal("accepted duplicate slug")
	}
	renamed, err := RenameBlock(root, "docs/draft", "03-opening", "New Heading", false)
	if err != nil || renamed.NewFilename != "03-new-heading.md" {
		t.Fatalf("rename %#v (%v)", renamed, err)
	}
	if got, _ := os.ReadFile(filepath.Join(root, "docs/draft/03-new-heading.md")); string(got) != "# Title\n" {
		t.Fatalf("renamed body %q", got)
	}
	removed, err := RemoveBlock(root, "docs/draft", "notes", true)
	if err != nil || removed.RemainingBlocks != 2 {
		t.Fatalf("remove preview %#v (%v)", removed, err)
	}
	if _, err := RemoveBlock(root, "docs/draft", "notes", false); err != nil {
		t.Fatal(err)
	}
	if _, err := RemoveBlock(root, "docs/draft", "middle", false); err != nil {
		t.Fatal(err)
	}
	if _, err := RemoveBlock(root, "docs/draft", "new-heading", true); err == nil {
		t.Fatal("preview accepted deleting last block")
	}
}

func ptrString(s string) *string { return &s }

func TestBlockRenameCollisionAndRemovalShape(t *testing.T) {
	root := canonicalPath(t.TempDir())
	putContent(t, root, "docs/draft/01-first.md", "first")
	putContent(t, root, "docs/draft/01-taken.md", "taken")
	if _, err := RenameBlock(root, "docs/draft", "first", "taken", false); err == nil {
		t.Fatal("rename overwrote collision")
	}
	if _, err := RenameBlock(root, "docs/draft", "first", "taken", true); err != nil {
		t.Fatal(err)
	}
	if got, _ := os.ReadFile(filepath.Join(root, "docs/draft/01-taken.md")); string(got) != "first" {
		t.Fatalf("forced rename body %q", got)
	}
	putContent(t, root, "docs/single.md", "body")
	if _, err := RemoveBlock(root, "docs/single.md", "single", true); err == nil || !strings.Contains(err.Error(), "not a directory article") {
		t.Fatal("single-file removal", err)
	}
}

func TestNewBlockDefaultTyporaAndRenumberPreview(t *testing.T) {
	root := canonicalPath(t.TempDir())
	putContent(t, root, "mind.yaml", "schema: '1'\n")
	putContent(t, root, "assets/placeholder", "")
	putContent(t, root, "docs/draft/05-opening.md", "# Title\n")
	if _, err := NewBlock(root, "docs/draft", "Next", BlockEditOptions{Start: 1}); err != nil {
		t.Fatal(err)
	}
	got, err := os.ReadFile(filepath.Join(root, "docs/draft/02-next.md"))
	if err != nil || string(got) != "---\ntypora-copy-images-to: ../../assets\n---\n\n# Next\n\n" {
		t.Fatalf("new body %q (%v)", got, err)
	}
	report, err := RenumberBlocks(root, "docs/draft", 8, true)
	if err != nil || !reflect.DeepEqual([]string(report.Order), []string{"08-opening.md", "09-next.md"}) {
		t.Fatalf("renumber %#v (%v)", report, err)
	}
	if _, err := os.Stat(filepath.Join(root, "docs/draft/01-opening.md")); err != nil {
		t.Fatal("renumber preview wrote", err)
	}
}
