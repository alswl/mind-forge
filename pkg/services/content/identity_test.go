package content

import (
	"errors"
	"os"
	"path/filepath"
	"testing"

	output "github.com/alswl/mind-forge/pkg/common/outcome"
)

func putContent(t *testing.T, root, relative, text string) {
	t.Helper()
	path := filepath.Join(root, filepath.FromSlash(relative))
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(text), 0o644); err != nil {
		t.Fatal(err)
	}
}

func TestIdentityRoundTrip(t *testing.T) {
	for _, text := range []string{"::idea", "::idea::draft", "projects/team::idea", "projects/team::idea::draft"} {
		identity, err := ParseIdentity(text)
		if err != nil || identity.String() != text {
			t.Errorf("%q: %#v (%v)", text, identity, err)
		}
	}
	for _, text := range []string{"idea", "p::::draft", "p::idea::", "p::idea::draft::extra", "../p::idea", "./p::idea", "/p::idea", "p::note.md", "p::.hidden", "p:: x"} {
		if _, err := ParseIdentity(text); err == nil {
			t.Errorf("accepted noncanonical identity %q", text)
		}
	}
}

func TestLocalPathSelectorsUseCWDAndPreserveBoundary(t *testing.T) {
	root, err := filepath.EvalSymlinks(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	cwd := filepath.Join(root, "docs")
	if err := os.MkdirAll(cwd, 0o755); err != nil {
		t.Fatal(err)
	}
	got, err := ResolvePathSelector("notes.md", cwd, &root, "project")
	if err != nil || got != filepath.Join(cwd, "notes.md") {
		t.Fatalf("path selector: %s (%v)", got, err)
	}
	for _, input := range []string{"", "../notes.md", "a/../notes.md", "a\\..\\notes.md"} {
		if _, err := ResolvePathSelector(input, cwd, &root, "project"); err == nil {
			t.Errorf("unsafe selector %q accepted", input)
		}
	}
	for _, kind := range []EntityKind{Article, Asset, Source} {
		identity, err := NormalizeEntitySelector(root, "projects/example", kind, "item.md")
		if err != nil || identity.ProjectIdentity != "projects/example" || identity.ResolvedPath != filepath.Join(root, identity.Path) {
			t.Fatalf("local identity: %#v (%v)", identity, err)
		}
	}
	if err := os.Symlink(t.TempDir(), filepath.Join(root, "escape")); err != nil {
		t.Fatal(err)
	}
	if err := ValidateEntityPath(root, "escape/new.md"); err == nil {
		t.Fatal("symlink escape was accepted")
	}
}

func TestProjectSelectionFromCanonicalIdentity(t *testing.T) {
	root := t.TempDir()
	putContent(t, root, "minds.yaml", "schema_version: '1'\nprojects_dir: projects\nprojects: [projects/first, projects/second]\n")
	for _, project := range []string{"", "projects/first", "projects/second"} {
		putContent(t, root, filepath.ToSlash(filepath.Join(project, "mind.yaml")), "schema_version: '1'\ncontent_layout: category\n")
	}
	for _, text := range []string{"::idea", "projects/second::idea::draft"} {
		got, err := ProjectFor(root, filepath.Join(root, "projects/first"), nil, &text)
		want := root
		if text != "::idea" {
			want = filepath.Join(root, "projects/second")
		}
		if err != nil || got != want {
			t.Errorf("%s selected %s, %v; want %s", text, got, err, want)
		}
	}
	selected, identity := "first", "projects/second::idea"
	if _, err := ProjectFor(root, root, &selected, &identity); err == nil {
		t.Fatal("explicit project conflict was guessed away")
	}
}

func TestOutputSelectionRefusesAmbiguityAndUsesExplicitDefault(t *testing.T) {
	root := t.TempDir()
	putContent(t, root, "mind.yaml", "schema_version: '1'\ncontent_layout: category\nminds:\n  idea: {default_output: final}\n  other: {}\n")
	for _, file := range []string{"outputs/idea/draft.md", "outputs/idea/final.md", "outputs/other/draft.md"} {
		putContent(t, root, file, "# Synthetic\n")
	}
	for selector, want := range map[string]string{"idea": "::idea::final", "::idea": "::idea::final", "idea::draft": "::idea::draft", "outputs/idea/draft.md": "::idea::draft", "::idea::draft": "::idea::draft"} {
		got, err := ResolveOutput(root, root, selector)
		if err != nil || got.Identity != want {
			t.Errorf("%q: %#v (%v)", selector, got, err)
		}
	}
	_, err := ResolveOutput(root, root, "draft")
	var failure *output.Error
	if !errors.As(err, &failure) || failure.Kind != "selection_ambiguous" || failure.Candidates == nil || len(*failure.Candidates) != 2 {
		t.Fatalf("ambiguous output: %#v", err)
	}
	putContent(t, root, "mind.yaml", "schema_version: '1'\ncontent_layout: category\nminds:\n  idea: {default_output: absent}\n")
	if _, err := ResolveOutput(root, root, "idea"); !errors.As(err, &failure) || failure.Kind != "not_found" || failure.Message != "default output missing: ::idea::absent" {
		t.Fatalf("missing default: %#v", err)
	}
}
