package content

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/alswl/mind-forge/internal/store"
)

func TestArticleCreationShapesPreviewAndIndexAuthority(t *testing.T) {
	for _, file := range []bool{false, true} {
		root := canonicalPath(t.TempDir())
		putContent(t, root, "mind.yaml", "schema: '1'\n")
		original := "schema: '1'\nsources:\n  stable: {name: stable, type: file, tags: [], added_at: ''}\nopaque: [unchanged] # literal\n"
		putContent(t, root, "mind-index.yaml", original)
		now := time.Date(2026, 2, 3, 4, 5, 6, 0, time.UTC)
		options := ArticleCreateOptions{File: file, Draft: true, DryRun: true, Template: "blog", Now: now}
		result, err := CreateArticle(root, "Weekly Report", options)
		if err != nil || len(result.Files) == 0 {
			t.Fatalf("preview %#v %v", result, err)
		}
		if _, err := os.Stat(filepath.Join(root, "docs")); !os.IsNotExist(err) {
			t.Fatal("preview created docs")
		}
		if got, _ := os.ReadFile(filepath.Join(root, "mind-index.yaml")); string(got) != original {
			t.Fatal("preview wrote index")
		}
		options.DryRun = false
		result, err = CreateArticle(root, "Weekly Report", options)
		if err != nil {
			t.Fatal(err)
		}
		articlePath := "docs/weekly-report"
		bodyPath := filepath.Join(root, articlePath, "01-opening.md")
		if file {
			articlePath += ".md"
			bodyPath = filepath.Join(root, articlePath)
		}
		body, err := os.ReadFile(bodyPath)
		if err != nil || !strings.Contains(string(body), "# Weekly Report\n\n> Created: 2026-02-03T04:05:06Z") {
			t.Fatalf("body %q %v", body, err)
		}
		index, err := store.ReadIndex(filepath.Join(root, "mind-index.yaml"), false)
		if err != nil || index.Value.Articles == nil || len(*index.Value.Articles) != 1 {
			t.Fatalf("index %#v %v", index, err)
		}
		article := (*index.Value.Articles)[0]
		if article.Path != articlePath || article.Title != "Weekly Report" || article.Type != "blog" || article.Status != "draft" {
			t.Fatalf("article %#v", article)
		}
		raw, _ := os.ReadFile(filepath.Join(root, "mind-index.yaml"))
		if !strings.Contains(string(raw), "opaque: [unchanged] # literal") || !strings.Contains(string(raw), "  stable: {name: stable, type: file, tags: [], added_at: ''}") {
			t.Fatal("unrelated authority changed", string(raw))
		}
		if _, err := CreateArticle(root, "Weekly Report", options); err == nil {
			t.Fatal("overwrite accepted")
		}
		options.File = !file
		options.Force = true
		if _, err := CreateArticle(root, "Weekly Report", options); err == nil {
			t.Fatal("forced cross-shape overwrite accepted")
		}
	}
}

func TestArticleTemplateRefusalAndForceRollback(t *testing.T) {
	root := canonicalPath(t.TempDir())
	putContent(t, root, "mind.yaml", "schema: '1'\nplugins:\n  typora-front-matter:\n    enabled: false\n")
	putContent(t, root, "mind-index.yaml", "schema: '1'\n")
	putContent(t, root, "templates/custom.md", "# {title}\n{tags}\n## Detail\n")
	slug := "verbatim_slug"
	result, err := CreateArticle(root, "Title", ArticleCreateOptions{Slug: &slug, Template: "templates/custom.md", Tags: []string{"a", "b"}, Draft: false})
	if err != nil || result.Filename != slug {
		t.Fatalf("custom %#v %v", result, err)
	}
	body, _ := os.ReadFile(filepath.Join(root, "docs", slug, "01-opening.md"))
	if string(body) != "# Title\na, b" {
		t.Fatalf("custom body %q", body)
	}
	for _, template := range []string{"../outside.md", "/outside.md", "missing.md"} {
		if _, err := CreateArticle(root, "Preview", ArticleCreateOptions{Template: template, DryRun: true}); err == nil {
			t.Fatalf("accepted %s", template)
		}
	}
	putContent(t, root, "mind-index.yaml", "broken: [\n")
	if _, err := CreateArticle(root, "Title", ArticleCreateOptions{Slug: &slug, Template: "blank", Force: true}); err == nil {
		t.Fatal("accepted malformed index")
	}
	after, _ := os.ReadFile(filepath.Join(root, "docs", slug, "01-opening.md"))
	if string(after) != string(body) {
		t.Fatal("failed creation replaced source")
	}
}

func TestArticlePublicationRestoresAuthorContentOnIndexFailure(t *testing.T) {
	for _, file := range []bool{false, true} {
		root := t.TempDir()
		path := filepath.Join(root, "author")
		if file {
			if err := os.WriteFile(path, []byte("before"), 0o600); err != nil {
				t.Fatal(err)
			}
		} else {
			if err := os.Mkdir(path, 0o700); err != nil {
				t.Fatal(err)
			}
			if err := os.WriteFile(filepath.Join(path, "original.md"), []byte("before"), 0o600); err != nil {
				t.Fatal(err)
			}
		}
		failure := errors.New("index publication refused")
		err := publishArticle(path, []byte("after"), map[string][]byte{"new.md": []byte("after")}, file, true, func() error { return failure }, func() bool { return false })
		if !errors.Is(err, failure) {
			t.Fatal("missing publication failure", err)
		}
		oldPath := path
		if !file {
			oldPath = filepath.Join(path, "original.md")
		}
		if got, err := os.ReadFile(oldPath); err != nil || string(got) != "before" {
			t.Fatalf("rollback %q %v", got, err)
		}
		if entries, _ := os.ReadDir(root); len(entries) != 1 {
			t.Fatal("staging or backup leaked", entries)
		}
	}
}

func TestArticlePublicationRefusesNewlyOccupiedDestinationWithoutForce(t *testing.T) {
	root := t.TempDir()
	path := filepath.Join(root, "article.md")
	if err := os.WriteFile(path, []byte("external author"), 0o644); err != nil {
		t.Fatal(err)
	}
	called := false
	err := publishArticle(path, []byte("new"), nil, true, false, func() error { called = true; return nil }, func() bool { return false })
	if err == nil || called {
		t.Fatal("occupied destination published", err, called)
	}
	body, _ := os.ReadFile(path)
	if string(body) != "external author" {
		t.Fatalf("overwrote author %q", body)
	}
}
