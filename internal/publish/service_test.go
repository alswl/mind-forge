package publish

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/alswl/mind-forge/internal/model"
)

func putPublish(t *testing.T, root, name, body string) {
	t.Helper()
	path := filepath.Join(root, name)
	if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(body), 0644); err != nil {
		t.Fatal(err)
	}
}

func TestPublishPayloadTransformsPreserveBuiltSource(t *testing.T) {
	root := t.TempDir()
	putPublish(t, root, "diagram.png", "PNG")
	target := model.PublishTarget{Config: map[string]any{"svg_to_png": true, "banner_markdown": "Notice\n"}}
	original := "![a](diagram.svg) ![b](missing.svg) ![web](https://host/a.svg)\n\n```md\n![literal](diagram.svg)\n```\n"
	got, report, err := Transforms(original, root, target, root)
	if err != nil || !strings.HasPrefix(got, "Notice\n\n") || !strings.Contains(got, "![a](diagram.png)") || len(report.SVGPNGReplaced) != 1 || len(report.SVGPNGMissing) != 1 || !report.BannerInjected {
		t.Fatalf("%q %+v %v", got, report, err)
	}
	if !strings.Contains(got, "![literal](diagram.svg)") || !strings.Contains(got, "https://host/a.svg") {
		t.Fatal(got)
	}
	twice, next, err := Transforms(got, root, target, root)
	if err != nil || twice != got || next.BannerInjected {
		t.Fatal("banner not idempotent", err)
	}
}

func TestPublishRecordDecisionKeepsExistingTimestamp(t *testing.T) {
	now := time.Date(2026, 11, 16, 0, 0, 0, 0, time.UTC)
	published := "published"
	url := "https://example.test/doc"
	first, action, err := Upsert(nil, "docs/post.md", "web", &published, &url, now)
	if err != nil || action != "created" || first.PublishedAt == nil {
		t.Fatalf("%+v %s %v", first, action, err)
	}
	archived := "archived"
	updated, action, err := Upsert(&first, first.Path, "web", &archived, nil, now.Add(time.Hour))
	if err != nil || action != "updated" || *updated.PublishedAt != *first.PublishedAt || *updated.TargetURL != url {
		t.Fatalf("%+v %v", updated, err)
	}
	if _, _, err := Upsert(nil, "post", "web", nil, &url, now); err == nil {
		t.Fatal("created without status")
	}
	if _, _, err := Upsert(nil, "post", "web", &archived, nil, now); err == nil {
		t.Fatal("initial archive without URL")
	}
}

func TestFilePublisherPrecedenceAndDiagnosticBlocking(t *testing.T) {
	root := t.TempDir()
	project := filepath.Join(root, "project")
	putPublish(t, root, ".mind-forge/publisher/desk.yaml", "type: local\nconfig: {path: exported, prefix: repo-}\n")
	name := "desk"
	config := model.DefaultMindConfig()
	config.Publish.DefaultTarget = &name
	targets := []model.PublishTarget{{Name: name, Type: "local", Enabled: true}}
	config.Publish.Targets = &targets
	resolved, err := ResolveTarget(root, project, &config, nil)
	if err != nil || resolved.Base != root || resolved.Target.Prefix == nil || *resolved.Target.Prefix != "repo-" {
		t.Fatalf("%+v %v", resolved, err)
	}
	putPublish(t, root, ".mind-forge/publisher/duplicate.yml", "name: desk\ntype: local\nconfig: {path: elsewhere}\n")
	if _, err := ResolveTarget(root, project, &config, &name); err == nil {
		t.Fatal("duplicate silently selected")
	}
}

func TestPublishLocalDatePathUsesTemplateOriginBeforeFilename(t *testing.T) {
	path := "out/{date:YYYY}/{date:YYYY-MM-DD}"
	target := model.PublishTarget{Name: "local", Path: &path}
	article := model.Article{Path: "docs/2026-01-01-post.md", TemplateOrigin: &model.TemplateOrigin{SlotValue: "2025-12-31"}}
	dir, date, err := LocalPath("/project", target, article)
	if err != nil || dir != "/project/out/2025/2025-12-31" || date == nil || date.Origin != "template_slot" {
		t.Fatalf("%q %+v %v", dir, date, err)
	}
	article.TemplateOrigin = nil
	article.Path = "docs/no-date.md"
	if _, _, err := LocalPath("/project", target, article); err == nil {
		t.Fatal("accepted date placeholder without date")
	}
}

func TestPublishPreviewCacheMissDoesNotMaterializeIndex(t *testing.T) {
	root := t.TempDir()
	putPublish(t, root, "mind.yaml", "schema: '1'\npublish:\n  default_target: desk\n  targets:\n  - name: desk\n    type: local\n    path: exported\n")
	putPublish(t, root, "docs/post.md", "# Author\n")
	putPublish(t, root, "outputs/post.md", "# Built\n")
	result, _, _, err := Run(root, root, "post", RunOptions{DryRun: true})
	if err != nil || result.Local == nil {
		t.Fatalf("preview: %+v %v", result, err)
	}
	if _, err := os.Stat(filepath.Join(root, "mind-index.yaml")); !os.IsNotExist(err) {
		t.Fatal("preview materialized cache index")
	}
	if _, err := os.Stat(filepath.Join(root, "exported")); !os.IsNotExist(err) {
		t.Fatal("preview materialized destination")
	}
}
