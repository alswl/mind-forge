package content

import (
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
	"time"

	"github.com/alswl/mind-forge/pkg/dal/store"
)

func mutationProject(t *testing.T, root, name string) string {
	t.Helper()
	project := filepath.Join(root, name)
	putContent(t, root, name+"/mind.yaml", "schema: '1'\n")
	putContent(t, root, name+"/mind-index.yaml", "schema: '1'\nopaque: [keep] # literal\n")
	return project
}

func TestArticleRenamePlansAuthorAndRoleChangesTogether(t *testing.T) {
	root := canonicalPath(t.TempDir())
	project := mutationProject(t, root, "alpha")
	putContent(t, project, "docs/old/01-opening.md", "# Author\n")
	putContent(t, project, "prompts/old.md", "---\narticle: docs/old\nmode: research\n---\nbody\n")
	putContent(t, project, "thinking/old.md", "private thought")
	putContent(t, project, "mind-index.yaml", "schema: '1'\narticles:\n  docs/old:\n    title: Original title\n    article_path: docs/old\n    created_at: first\n    updated_at: previous\nopaque: [keep] # literal\n")
	mutation, err := PlanArticleRename(root, project, "docs/old", "New Slug", false, time.Date(2026, 2, 3, 4, 5, 6, 0, time.UTC))
	if err != nil {
		t.Fatal(err)
	}
	after := mutation.Plan.After
	if string(after["alpha/docs/new-slug/01-opening.md"].Bytes) != "# Author\n" {
		t.Fatal("author not relocated")
	}
	if _, ok := after["alpha/docs/old/01-opening.md"]; ok {
		t.Fatal("old author retained")
	}
	if !strings.Contains(string(after["alpha/prompts/new-slug.md"].Bytes), "article: docs/new-slug\n") {
		t.Fatal("prompt binding not rewritten")
	}
	index, err := store.DecodeIndex(after["alpha/mind-index.yaml"].Bytes, "planned", false)
	if err != nil {
		t.Fatal(err)
	}
	article := (*index.Value.Articles)[0]
	if article.Title != "Original title" || article.Path != "docs/new-slug" || article.CreatedAt != "first" {
		t.Fatalf("article %#v", article)
	}
	if (*index.Value.Thinking)[0].Article != "docs/new-slug" {
		t.Fatal("thinking projection stale")
	}
	if raw, _ := os.ReadFile(filepath.Join(project, "prompts/old.md")); !strings.Contains(string(raw), "article: docs/old") {
		t.Fatal("planning changed source")
	}
	if _, err := os.Stat(filepath.Join(root, ".mind-forge")); !os.IsNotExist(err) {
		t.Fatal("planning created journal")
	}
}

func TestArticleMovePlansBothIndicesAndBoundFiles(t *testing.T) {
	root := canonicalPath(t.TempDir())
	source := mutationProject(t, root, "source")
	target := mutationProject(t, root, "target")
	putContent(t, source, "docs/post.md", "# Post\n")
	putContent(t, source, "prompts/control.md", "---\narticle: docs/post.md\n---\n")
	putContent(t, source, "mind-index.yaml", "schema: '1'\narticles:\n  docs/post:\n    title: Post\n    project: source\n    article_path: docs/post.md\n    created_at: first\nprompts:\n  control: {path: prompts/control.md, article: docs/post.md, updated_at: old}\npublish_records:\n  - {path: docs/post.md, target_name: local, status: published}\nopaque: [keep] # literal\n")
	mutation, err := PlanArticleMove(root, source, target, "Post", false)
	if err != nil {
		t.Fatal(err)
	}
	if string(mutation.Plan.After["target/docs/post.md"].Bytes) != "# Post\n" {
		t.Fatal("author not moved")
	}
	if _, ok := mutation.Plan.After["source/docs/post.md"]; ok {
		t.Fatal("source retained")
	}
	dst, err := store.DecodeIndex(mutation.Plan.After["target/mind-index.yaml"].Bytes, "target", false)
	if err != nil {
		t.Fatal(err)
	}
	if len(*dst.Value.Articles) != 1 || len(*dst.Value.Prompts) != 1 || len(*dst.Value.PublishRecords) != 1 {
		t.Fatal("metadata not transferred")
	}
	src, err := store.DecodeIndex(mutation.Plan.After["source/mind-index.yaml"].Bytes, "source", false)
	if err != nil {
		t.Fatal(err)
	}
	if sliceLength(src.Value.Articles) != 0 || sliceLength(src.Value.Prompts) != 0 || sliceLength(src.Value.PublishRecords) != 0 {
		t.Fatal("source projections retained")
	}
	if !strings.Contains(string(mutation.Plan.After["source/mind-index.yaml"].Bytes), "opaque: [keep] # literal") {
		t.Fatal("opaque data lost")
	}
}

func TestArticleRemoveUsesExactSelectorAndPreservesOtherRegistrations(t *testing.T) {
	root := canonicalPath(t.TempDir())
	project := mutationProject(t, root, "alpha")
	putContent(t, project, "docs/monthly.md", "# Author\n")
	putContent(t, project, "mind-index.yaml", "schema: '1'\narticles:\n  docs/monthly:\n    title: Monthly\n    article_path: docs/monthly.md\nsources:\n  stable: {name: stable, type: file, tags: [], added_at: ''}\nopaque: [keep] # literal\n")
	mutation, err := PlanArticleRemove(root, project, "monthly", false)
	if err != nil {
		t.Fatal(err)
	}
	if _, ok := mutation.Plan.After["alpha/docs/monthly.md"]; ok {
		t.Fatal("author retained")
	}
	if !strings.Contains(string(mutation.Plan.After["alpha/mind-index.yaml"].Bytes), "stable: {name: stable, type: file, tags: [], added_at: ''}") {
		t.Fatal("authority changed")
	}
	if _, err := PlanArticleRemove(root, project, "month", false); err == nil {
		t.Fatal("substring accepted")
	}
}

func TestArticlePlansRefuseCollisionsAndLeaveDryMoveReadOnly(t *testing.T) {
	root := canonicalPath(t.TempDir())
	source := mutationProject(t, root, "source")
	target := mutationProject(t, root, "target")
	putContent(t, source, "docs/post.md", "source")
	putContent(t, source, "docs/taken.md", "target")
	putContent(t, source, "mind-index.yaml", "schema: '1'\narticles:\n  docs/post: {title: Post, article_path: docs/post.md}\n")
	if _, err := PlanArticleRename(root, source, "Post", "taken", false, time.Time{}); err == nil {
		t.Fatal("rename collision accepted")
	}
	forced, err := PlanArticleRename(root, source, "Post", "taken", true, time.Time{})
	if err != nil {
		t.Fatal(err)
	}
	if string(forced.Plan.After["source/docs/taken.md"].Bytes) != "source" {
		t.Fatal("force did not replace target in plan")
	}
	putContent(t, target, "docs/post.md", "occupied")
	if _, err := PlanArticleMove(root, source, target, "Post", false); err == nil {
		t.Fatal("move collision accepted")
	}
	preview, err := PlanArticleMove(root, source, target, "Post", true)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(preview.Plan.Before, preview.Plan.After) {
		t.Fatal("legacy move preview planned writes")
	}
	if raw, _ := os.ReadFile(filepath.Join(source, "docs/post.md")); string(raw) != "source" {
		t.Fatal("planning changed author content")
	}
}
