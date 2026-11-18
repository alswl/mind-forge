package content

import (
	"errors"
	"os"
	"path/filepath"
	"testing"

	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
)

func TestDiscoveryPreservesUnicodeCollisionRules(t *testing.T) {
	for _, names := range [][2]string{{"Note", "note"}, {"İ", "i\u0307"}, {"ΟΣ", "Ος"}, {"\u1c89", "\u1c8a"}} {
		root := t.TempDir()
		putContent(t, root, "mind.yaml", "schema_version: '1'\ncontent_layout: category\nminds:\n  "+names[0]+": {}\n  "+names[1]+": {}\n")
		if _, err := Discover(root, root); err == nil {
			t.Errorf("case collision %q / %q accepted", names[0], names[1])
		}
	}
	config := model.DefaultMindConfig()
	if err := ValidateMindName(&config, "ſources"); err != nil {
		t.Fatalf("reserved names use ASCII equality, not Unicode EqualFold: %v", err)
	}
}

func TestOutputNamedLikeAnotherMindRequiresExactIdentity(t *testing.T) {
	root := t.TempDir()
	putContent(t, root, "mind.yaml", "schema_version: '1'\ncontent_layout: category\nminds: {idea: {}, other: {}}\n")
	putContent(t, root, "outputs/idea/final.md", "# Draft\n")
	putContent(t, root, "outputs/other/idea.md", "# Other\n")
	_, err := ResolveOutput(root, root, "idea")
	var failure *output.Error
	if !errors.As(err, &failure) || failure.Kind != "selection_ambiguous" || failure.Candidates == nil || len(*failure.Candidates) != 3 || (*failure.Candidates)[0] != "::idea" {
		t.Fatalf("mind/output collision was guessed: %v", err)
	}
	got, err := ResolveOutput(root, root, "::idea")
	if err != nil || got.Identity != "::idea::final" {
		t.Fatalf("exact mind identity: %#v (%v)", got, err)
	}
}

func TestDiscoveryKeepsZeroOutputMindsAndActualRoles(t *testing.T) {
	for _, layout := range []string{"category", "bundle"} {
		t.Run(layout, func(t *testing.T) {
			root := t.TempDir()
			putContent(t, root, "mind.yaml", "schema_version: '1'\ncontent_layout: "+layout+"\nminds:\n  empty: {title: Empty idea}\n  idea: {}\n")
			outputRoot, prompt := "outputs/idea", "prompts/idea.md"
			if layout == "bundle" {
				outputRoot, prompt = "idea/outputs", "idea/prompt.md"
			}
			putContent(t, root, outputRoot+"/draft.md", "# Draft\n")
			putContent(t, root, prompt, "# Prompt\n")
			minds, err := Discover(root, root)
			if err != nil || len(minds) != 2 {
				t.Fatalf("%#v (%v)", minds, err)
			}
			if minds[0].Identity != "::empty" || minds[0].Path != nil || len(minds[0].Outputs) != 0 || len(minds[0].Roles) != 0 || minds[0].Title != "Empty idea" {
				t.Fatalf("registration-only mind invented content: %#v", minds[0])
			}
			mind := minds[1]
			if len(mind.Roles) != 1 || mind.Roles["prompt"] != prompt || len(mind.Outputs) != 1 {
				t.Fatalf("roles/output: %#v", mind)
			}
			if mind.Outputs[0].Identity != "::idea::draft" || mind.Outputs[0].Path != outputRoot+"/draft.md" || mind.Outputs[0].BuildPath != "build/idea/draft.md" {
				t.Fatal(mind.Outputs[0])
			}
			if layout == "bundle" && (mind.Path == nil || *mind.Path != "idea") {
				t.Fatalf("existing bundle path: %#v", mind.Path)
			}
		})
	}
}

func TestDiscoveryExcludesChildProjectsAndReservedDirectories(t *testing.T) {
	root := t.TempDir()
	putContent(t, root, "minds.yaml", "schema_version: '1'\nprojects_dir: projects\nprojects: [nested/child]\n")
	putContent(t, root, "mind.yaml", "schema_version: '1'\ncontent_layout: bundle\nreserved_roots: [scratch]\n")
	putContent(t, root, "nested/child/mind.yaml", "schema_version: '1'\ncontent_layout: category\n")
	for _, path := range []string{"scratch/note.md", ".hidden/note.md", "build/out.md", "projects/not-a-mind/note.md", "idea/outputs/final/content.md"} {
		putContent(t, root, path, "# Example\n")
	}
	minds, err := Discover(root, root)
	if err != nil || len(minds) != 1 || minds[0].Name != "idea" {
		t.Fatalf("child/derived paths became minds: %#v (%v)", minds, err)
	}
	if minds[0].Outputs[0].Shape != "directory" {
		t.Fatal(minds[0].Outputs)
	}
	nested := filepath.Join(root, "nested/child")
	putContent(t, nested, "outputs/topic/article.md", "# Example\n")
	minds, err = Discover(root, nested)
	if err != nil || len(minds) != 1 || minds[0].Identity != "nested/child::topic" {
		t.Fatalf("nested identity: %#v (%v)", minds, err)
	}
}

func TestDiscoveryRefusesLegacyAndUnsafeOwnership(t *testing.T) {
	root := t.TempDir()
	putContent(t, root, "mind.yaml", "schema_version: '1'\n")
	if _, err := Discover(root, root); err == nil {
		t.Fatal("legacy project acquired inferred ownership")
	}
	putContent(t, root, "mind.yaml", "schema_version: '1'\ncontent_layout: category\nminds: {cache: {}}\n")
	if _, err := Discover(root, root); err == nil {
		t.Fatal("reserved name was accepted")
	}
	putContent(t, root, "mind.yaml", "schema_version: '1'\ncontent_layout: category\n")
	putContent(t, root, "outputs/idea/final.md", "# Example\n")
	putContent(t, root, "outputs/idea/final/section.md", "# Example\n")
	_, err := Discover(root, root)
	var failure *output.Error
	if !errors.As(err, &failure) || failure.Kind != "shape_conflict" {
		t.Fatalf("same-name shapes were accepted: %v", err)
	}
	if err := os.RemoveAll(filepath.Join(root, "outputs")); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Join(root, "outputs"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(t.TempDir(), filepath.Join(root, "outputs/linked")); err != nil {
		t.Fatal(err)
	}
	if _, err := Discover(root, root); err == nil {
		t.Fatal("symlink content was accepted")
	}
}
