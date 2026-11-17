package source

import (
	"errors"
	"testing"

	"github.com/alswl/mind-forge/internal/model"
	"github.com/alswl/mind-forge/internal/output"
)

func TestPathNameAndFileKindFollowReferenceStemRules(t *testing.T) {
	for _, test := range []struct{ path, name, kind string }{{"notes/report.md", "report", "file"}, {"notes/report.PDF", "report", "pdf"}, {"notes/archive.tar.gz", "archive.tar", "file"}, {".pdf", ".pdf", "file"}, {".hidden.pdf", ".hidden", "pdf"}, {"notes/机器.PdF", "机器", "pdf"}, {"notes/report.", "report", "file"}, {"notes/folder/", "folder", "file"}, {"notes/folder/.", "folder", "file"}, {"C:\\folder\\report.pdf", "C:\\folder\\report", "pdf"}} {
		got, err := DeriveNameFromPath(test.path)
		if err != nil || got != test.name || InferFileKind(test.path) != test.kind {
			t.Fatalf("path %q: %q kind %s err %v", test.path, got, InferFileKind(test.path), err)
		}
	}
	for _, path := range []string{"", ".", "..", "/", "notes/..", "notes/folder/..", "notes/folder/../."} {
		if _, err := DeriveNameFromPath(path); err == nil {
			t.Fatalf("derived name from %q", path)
		}
	}
}

func TestURLFileExtensionRetainsLiteralSuffixAndFallback(t *testing.T) {
	for _, test := range []struct{ url, kind, want string }{{"https://example.test/a.PDF?q=.xml#part", "web", "PDF"}, {"https://example.test/a.rss#part?x=1", "rss", "rss"}, {"https://example.test/feed", "rss", "xml"}, {"https://example.test/feed", "web", "html"}, {"https://example.test/a.12345678901", "web", "html"}, {"https://example.test/.rss", "rss", "xml"}, {"https://example.test", "web", "test"}, {"https://example.test/a.", "web", "html"}} {
		if got := FileExtensionForURL(test.url, test.kind); got != test.want {
			t.Fatalf("%q kind %s: %q want %q", test.url, test.kind, got, test.want)
		}
	}
}

func TestUniqueNameSuggestionUsesFirstScopedSegmentThenParent(t *testing.T) {
	for _, test := range []struct{ path, root, want string }{{"sources/archive/2026-07/0731.md", "sources", "archive-0731"}, {"sources/report.md", "sources", "sources-report"}, {"sources/report/report.md", "sources", "report-report"}, {"sources/机器/装置.md", "sources", "机器-装置"}, {"/external/report.md", "/project/sources", "/-report"}} {
		if got := SuggestUniqueName(test.path, test.root); got != test.want {
			t.Fatalf("%q scope %q: %q want %q", test.path, test.root, got, test.want)
		}
	}
}

func TestSourceSelectorUsesAuthorityAndRefusesAmbiguousNamesOrPaths(t *testing.T) {
	pathA, pathB := "sources/a.md", "sources/b.md"
	items := []model.Source{{Name: "first", Path: &pathA}, {Name: "sources/a.md", Path: &pathB}, {Name: "url-only"}}
	for _, selector := range []string{"first", "sources/b.md", "url-only"} {
		if _, err := ResolveSelector(items, selector); err != nil {
			t.Fatalf("exact selector %q: %v", selector, err)
		}
	}
	_, err := ResolveSelector(items, "sources/a.md")
	var failure *output.Error
	if !errors.As(err, &failure) || failure.Kind != "usage" || failure.Hint == nil || *failure.Hint != "use an unambiguous source name or path" {
		t.Fatal(err)
	}
	for _, selector := range []string{"fir", "missing"} {
		_, err := ResolveSelector(items, selector)
		if !errors.As(err, &failure) || failure.Kind != "not_found" || failure.Hint == nil || *failure.Hint != "use `mf source list` to see available sources" {
			t.Fatal(err)
		}
	}
}
