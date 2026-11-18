package content

import (
	"errors"
	"os"
	"path/filepath"
	"regexp"
	"testing"

	output "github.com/alswl/mind-forge/pkg/common/outcome"
)

func TestOriginalErrorVariants(t *testing.T) {
	_, placeholder := ParsePathTemplate("{x}")
	template, err := ParsePathTemplate("{date:YYYY}/{date:YYYY}.md")
	if err != nil {
		t.Fatal(err)
	}
	multiple := template.ValidateSlots("t")
	_, unknown := CreateArticle(t.TempDir(), "x", ArticleCreateOptions{Template: "x", DryRun: true})
	_, duplicate := SplitArticleTemplate("## x\nfirst\n## X\nsecond\n")
	root := t.TempDir()
	if err := os.MkdirAll(filepath.Join(root, "docs"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, "docs/x.md"), []byte("existing"), 0o644); err != nil {
		t.Fatal(err)
	}
	_, shape := CreateArticle(root, "x", ArticleCreateOptions{DryRun: true})
	for _, row := range []struct {
		name string
		err  error
		kind string
	}{{"UnknownPlaceholder", placeholder, "unknown_placeholder"}, {"MultiSlotTemplate", multiple, "multi_slot_template"}, {"UnknownTemplate", unknown, "unknown_template"}, {"DuplicateBlockSlug", duplicate, "duplicate_block_slug"}, {"ShapeConflict", shape, "shape_conflict"}} {
		t.Run(row.name, func(t *testing.T) {
			var failure *output.Error
			if !errors.As(row.err, &failure) || failure.Kind != row.kind || !regexp.MustCompile(`^[a-z][a-z0-9_]*$`).MatchString(failure.Kind) {
				t.Fatal(row.err)
			}
			if failure.Hint == nil {
				t.Fatalf("hint absent for %s", row.name)
			}
		})
	}
}
