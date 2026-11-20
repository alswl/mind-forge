package content

import (
	"errors"
	"strings"
	"testing"
	"time"

	"github.com/alswl/mind-forge/pkg/common/outcome"
	"github.com/alswl/mind-forge/pkg/dal/store"
)

func TestArticleConversionEligibilityAndStableOrder(t *testing.T) {
	root := canonicalPath(t.TempDir())
	project := mutationProject(t, root, "alpha")
	putContent(t, project, "docs/multi/01-opening.md", "# Title\n")
	putContent(t, project, "docs/multi/02-body.md", "## Body\n")
	putContent(t, project, "docs/single.md", "# Single\n")
	inspected, err := InspectArticleConversions(root, project, []string{"docs/single.md", "docs/multi"}, "to_single_file", false)
	if err != nil {
		t.Fatal(err)
	}
	if inspected[0].SourcePath != "docs/multi" || inspected[0].Reason == nil || *inspected[0].Reason != "multiple_section_files" {
		t.Fatalf("inspection %#v", inspected)
	}
	inspected, err = InspectArticleConversions(root, project, []string{"docs/multi"}, "to_single_file", true)
	if err != nil || !inspected[0].Eligible {
		t.Fatalf("merge %#v %v", inspected, err)
	}
	putContent(t, project, "docs/multi/extra.txt", "keep")
	inspected, err = InspectArticleConversions(root, project, []string{"docs/multi"}, "to_single_file", true)
	if err != nil || inspected[0].Reason == nil || *inspected[0].Reason != "extra_files" {
		t.Fatalf("extra %#v %v", inspected, err)
	}
}

func TestArticleConversionPlansRetainedSiblingAndBinding(t *testing.T) {
	root := canonicalPath(t.TempDir())
	project := mutationProject(t, root, "alpha")
	putContent(t, project, "mind-index.yaml", "schema: '1'\narticles:\n  post:\n    title: Post\n    article_path: docs/post/\n    created_at: first\nprompts:\n  post: {path: prompts/post.md, article: docs/post/, updated_at: previous}\nopaque: [keep] # literal\n")
	putContent(t, project, "docs/post/01-opening.md", "---\ntypora-copy-images-to: ../../assets\n---\n# Post\n![a](../../assets/a.png)\n")
	putContent(t, project, "docs/post/02-body.md", "## Body\n")
	putContent(t, project, "prompts/post.md", "---\narticle: docs/post/\n---\n")
	inspected, err := InspectArticleConversions(root, project, []string{"docs/post/"}, "to_single_file", true)
	if err != nil {
		t.Fatal(err)
	}
	item := inspected[0]
	if item.SourcePath != "docs/post/" || item.TargetPath != "docs/post.md" {
		t.Fatalf("D003 identity %#v", item)
	}
	mutation, err := PlanArticleConversion(root, project, item, time.Time{})
	if err != nil {
		t.Fatal(err)
	}
	body := string(mutation.Plan.After["alpha/docs/post.md"].Bytes)
	if !strings.Contains(body, "![a](../assets/a.png)") || !strings.Contains(body, "## Body\n") || strings.Contains(body, "typora-copy-images-to") {
		t.Fatalf("merge %q", body)
	}
	if _, exists := mutation.Plan.After["alpha/docs/post"]; exists {
		t.Fatal("old directory retained")
	}
	if !strings.Contains(string(mutation.Plan.After["alpha/prompts/post.md"].Bytes), "article: docs/post.md\n") {
		t.Fatal("prompt binding stale")
	}
	index, err := store.DecodeIndex(mutation.Plan.After["alpha/mind-index.yaml"].Bytes, "planned", false)
	if err != nil {
		t.Fatal(err)
	}
	if (*index.Value.Articles)[0].CreatedAt != "first" || (*index.Value.Articles)[0].Path != "docs/post.md" {
		t.Fatal("metadata lost")
	}
}

func TestArticleToDirectoryKeepsBytesInOpeningBlock(t *testing.T) {
	root := canonicalPath(t.TempDir())
	project := mutationProject(t, root, "alpha")
	putContent(t, project, "docs/post.md", "# Post\r\nbody\r\n")
	putContent(t, project, "mind-index.yaml", `schema: '1'
articles:
  post: {title: Post, article_path: docs/post.md}
publish_records:
  - {path: docs/post.md, target_name: web, status: published, target_url: "https://example.invalid/post", published_at: "2026-01-01T00:00:00Z"}
  - {path: docs/post.md, target_name: desk, status: draft}
  - {path: docs/another.md, target_name: web, status: draft}
`)
	items, err := InspectArticleConversions(root, project, []string{"docs/post.md"}, "to_directory", false)
	if err != nil {
		t.Fatal(err)
	}
	mutation, err := PlanArticleConversion(root, project, items[0], time.Time{})
	if err != nil {
		t.Fatal(err)
	}
	if body := string(mutation.Plan.After["alpha/docs/post/01-opening.md"].Bytes); body != "# Post\r\nbody\r\n" {
		t.Fatalf("bytes %q", body)
	}
	index, err := store.DecodeIndex(mutation.Plan.After["alpha/mind-index.yaml"].Bytes, "planned", false)
	if err != nil {
		t.Fatal(err)
	}
	records := *index.Value.PublishRecords
	if len(records) != 3 || records[0].Path != "docs/post" || records[1].Path != "docs/post" || records[2].Path != "docs/another.md" {
		t.Fatalf("publish record paths after conversion: %q", mutation.Plan.After["alpha/mind-index.yaml"].Bytes)
	}
	if record := records[0]; record.TargetName != "web" || record.Status != "published" ||
		record.TargetURL == nil || *record.TargetURL != "https://example.invalid/post" ||
		record.PublishedAt == nil || *record.PublishedAt != "2026-01-01T00:00:00Z" {
		t.Fatalf("lost publication metadata: %+v", record)
	}
}

func TestArticleConversionRefusesConflictingPublishRecords(t *testing.T) {
	root := canonicalPath(t.TempDir())
	project := mutationProject(t, root, "alpha")
	putContent(t, project, "docs/post.md", "# Post\n")
	putContent(t, project, "mind-index.yaml", `schema: '1'
articles:
  post: {title: Post, article_path: docs/post.md}
publish_records:
  - {path: docs/post.md, target_name: web, status: published}
  - {path: docs/post/, target_name: web, status: draft}
`)
	items, err := InspectArticleConversions(root, project, []string{"docs/post.md"}, "to_directory", false)
	if err != nil {
		t.Fatal(err)
	}
	_, err = PlanArticleConversion(root, project, items[0], time.Time{})
	var conflict *outcome.Error
	if !errors.As(err, &conflict) || conflict.Kind != "content_conflict" {
		t.Fatalf("expected a publication conflict, got %v", err)
	}
}

func TestLegacyArticleConversionInspection(t *testing.T) {
	root := canonicalPath(t.TempDir())
	project := mutationProject(t, root, "alpha")
	putContent(t, project, "docs/my-article/01-opening.md", "# Opening\n")
	putContent(t, project, "docs/my-article/02-body.md", "## Body\n")
	without, err := InspectArticleConversions(root, project, []string{"docs/my-article"}, "to_single_file", false)
	if err != nil || len(without) != 1 {
		t.Fatalf("inspection = %+v, %v", without, err)
	}
	if without[0].Eligible || without[0].Reason == nil || *without[0].Reason != "multiple_section_files" || len(without[0].SectionFiles) != 0 {
		t.Fatalf("without merge = %+v", without[0])
	}
	with, err := InspectArticleConversions(root, project, []string{"docs/my-article"}, "to_single_file", true)
	if err != nil || len(with) != 1 {
		t.Fatalf("inspection = %+v, %v", with, err)
	}
	if !with[0].Eligible || strings.Join(with[0].SectionFiles, "|") != "docs/my-article/01-opening.md|docs/my-article/02-body.md" || with[0].TargetPath != "docs/my-article.md" {
		t.Fatalf("with merge = %+v", with[0])
	}
}
