package content

import (
	"strings"
	"testing"
	"time"

	"github.com/alswl/mind-forge/internal/store"
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
	putContent(t, project, "mind-index.yaml", "schema: '1'\narticles:\n  post: {title: Post, article_path: docs/post.md}\n")
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
