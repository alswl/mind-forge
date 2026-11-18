package content

import (
	"errors"
	"github.com/alswl/mind-forge/internal/output"
	"strings"
	"testing"
)

func TestLegacyTemplateErrors(t *testing.T) {
	t.Run("unknown_placeholder_kind_and_hint", func(t *testing.T) {
		_, err := ParsePathTemplate("{foo}")
		var failure *output.Error
		if !errors.As(err, &failure) || failure.Kind != "unknown_placeholder" || failure.Hint == nil || !strings.Contains(*failure.Hint, "supported placeholders") {
			t.Fatalf("unknown placeholder = %v", err)
		}
	})
	t.Run("multi_slot_template_kind_and_hint", func(t *testing.T) {
		template, err := ParsePathTemplate("{date:YYYY}/{date:YYYY}.md")
		if err != nil {
			t.Fatal(err)
		}
		err = template.ValidateSlots("test")
		var failure *output.Error
		if !errors.As(err, &failure) || failure.Kind != "multi_slot_template" || failure.ExitCode() != 1 || failure.Hint == nil || !strings.Contains(*failure.Hint, "single distinguishing slot") {
			t.Fatalf("redundant slots = %v", err)
		}
	})
	t.Run("duplicate_block_slug_kind_and_exit_code", func(t *testing.T) {
		_, err := SplitArticleTemplate("## Notes\nfirst\n## NOTES\nsecond\n")
		var failure *output.Error
		if !errors.As(err, &failure) || failure.Kind != "duplicate_block_slug" || failure.ExitCode() != 1 || !strings.Contains(failure.Message, "notes") {
			t.Fatalf("duplicate heading = %v", err)
		}
	})
}

func TestLegacyArticleCreationErrors(t *testing.T) {
	t.Run("unknown_template_kind_and_exit_code", func(t *testing.T) {
		_, err := CreateArticle(t.TempDir(), "Test", ArticleCreateOptions{Template: "nope", DryRun: true})
		var failure *output.Error
		if !errors.As(err, &failure) || failure.Kind != "unknown_template" || failure.ExitCode() != 2 || !strings.Contains(failure.Message, "nope") {
			t.Fatalf("unknown template = %v", err)
		}
	})
	t.Run("shape_conflict_kind_and_exit_code", func(t *testing.T) {
		project := t.TempDir()
		putContent(t, project, "docs/test.md", "existing author text")
		_, err := CreateArticle(project, "Test", ArticleCreateOptions{DryRun: true})
		var failure *output.Error
		if !errors.As(err, &failure) || failure.Kind != "shape_conflict" || failure.ExitCode() != 1 || !strings.Contains(failure.Message, "directory") || !strings.Contains(failure.Message, "file") {
			t.Fatalf("shape conflict = %v", err)
		}
	})
}
