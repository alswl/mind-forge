package lifecycle

import (
	"errors"
	"testing"

	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
)

func TestOwnedInventoryRequiresKnownMindAndPreservesZeroOutputRegistrations(t *testing.T) {
	category := "category"
	config := model.DefaultMindConfig()
	config.ContentLayout = &category
	minds := []model.Mind{{Name: "brief", Identity: "p::brief"}, {Name: "zero", Identity: "p::zero"}}
	files := Snapshot{"reviews/brief.md": {Kind: "file"}, "outputs/brief": {Kind: "directory"}, "outputs/brief/a.md": {Kind: "file"}, "prompts/zero.md": {Kind: "file"}, "reviews/.hidden.md": {Kind: "file"}}
	owned, err := CollectOwnedRoots(config, minds, nil, files)
	if err != nil || len(owned) != 3 {
		t.Fatalf("owned inventory: %#v (%v)", owned, err)
	}
	mapping := OwnedMapping(owned, "bundle", map[string]string{"brief": "brief", "zero": "zero"})
	if mapping["prompts/zero.md"] != "zero/prompt.md" || mapping["reviews/brief.md"] != "brief/reviews.md" || mapping["outputs/brief"] != "brief/outputs" {
		t.Fatal(mapping)
	}
	for _, path := range []string{"reviews/unknown.md", "sources/loose.pdf", "docs/brief.md", "prompt/brief", "Sources/brief"} {
		candidate := cloneSnapshot(files)
		candidate[path] = Entry{Kind: "file"}
		var typed *output.Error
		if _, err := CollectOwnedRoots(config, minds, nil, candidate); !errors.As(err, &typed) || typed.ExitCode() != 2 {
			t.Fatalf("unresolved ownership/shape did not return usage error: %s (%v)", path, err)
		}
	}
}

func TestOwnedInventoryRejectsConfiguredLegacyRootsAndShapeCollisions(t *testing.T) {
	bundle := "bundle"
	config := model.DefaultMindConfig()
	config.ContentLayout = &bundle
	minds := []model.Mind{{Name: "brief", Identity: "p::brief"}}
	for _, files := range []Snapshot{
		{"brief/reviews.md": {Kind: "file"}, "brief/reviews": {Kind: "directory"}},
		{"brief/sources.md": {Kind: "file"}},
	} {
		if _, err := CollectOwnedRoots(config, minds, nil, files); err == nil {
			t.Fatal("conflicting role shape accepted")
		}
	}
	custom := "evidence"
	config.Layout.Sources = &custom
	if _, err := CollectOwnedRoots(config, minds, nil, Snapshot{"evidence/a.md": {Kind: "file"}}); err == nil {
		t.Fatal("custom legacy core root assigned implicitly")
	}
}
