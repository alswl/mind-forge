package content

import (
	"bytes"
	"os"
	"path/filepath"
	"testing"

	"github.com/alswl/mind-forge/pkg/dal/store"
)

func TestProjectLintPreviewsFixesAndPreservesUnrelatedSections(t *testing.T) {
	root := canonicalPath(t.TempDir())
	putContent(t, root, "mind.yaml", "schema: '1'\n")
	putContent(t, root, "mind-index.yaml", "schema: '1'\nassets:\n  missing:\n    name: missing\n    path: assets/missing.png\n    type: image\npublish_records:\n- path: output.md\n  target_name: archive\n  status: published\n  target_url: null\n  published_at: null\ncustom: {note: retained}\n")
	before, _ := os.ReadFile(filepath.Join(root, "mind-index.yaml"))
	issues, summary, err := LintProject(root, []string{"missing_directory", "stale_index_entry"}, false)
	if err != nil || len(issues) != 4 || summary.Errors != 3 || summary.Warnings != 1 {
		t.Fatalf("%#v %#v %v", issues, summary, err)
	}
	after, _ := os.ReadFile(filepath.Join(root, "mind-index.yaml"))
	if !bytes.Equal(before, after) {
		t.Fatal("read-only lint wrote")
	}
	issues, summary, err = LintProject(root, []string{"missing_directory", "stale_index_entry"}, true)
	if err != nil || len(issues) != 4 || summary.Fixed != 4 || summary.Errors != 0 || summary.Warnings != 0 {
		t.Fatalf("%#v %#v %v", issues, summary, err)
	}
	after, _ = os.ReadFile(filepath.Join(root, "mind-index.yaml"))
	if !bytes.Contains(after, []byte("publish_records:\n- path: output.md")) || !bytes.Contains(after, []byte("custom: {note: retained}")) {
		t.Fatal("cleanup lost unrelated authority")
	}
}

func TestProjectLintRoleBindingsAndConfiguredDirectory(t *testing.T) {
	root := canonicalPath(t.TempDir())
	putContent(t, root, "mind.yaml", "schema: '1'\ncontent_layout: category\nbuild:\n  articles:\n    declared:\n      article_dir: absent\n")
	putContent(t, root, "mind-index.yaml", "schema: '1'\narticles:\n- title: Post\n  type: blog\n  status: draft\n  article_path: docs/post.md\nprompts:\n- path: prompts/one.md\n  article: docs/post.md\n- path: prompts/two.md\n  article: docs/post.md\n- path: prompts/orphan.md\n  article: docs/absent.md\n- path: prompts/empty.md\n  article: ''\n")
	issues, summary, err := LintProject(root, []string{"missing_directory", "orphan_prompt", "duplicate_binding", "missing_thinking"}, false)
	if err != nil || len(issues) != 4 || summary.Errors != 4 || summary.Unfixed != 4 {
		t.Fatalf("%#v %#v %v", issues, summary, err)
	}
	// A duplicated binding does not also count as a bound prompt needing thinking.
	for _, issue := range issues {
		if issue.Kind == "missing_thinking" {
			t.Fatal("duplicate counted as bound")
		}
	}
	doc, err := store.ReadIndex(filepath.Join(root, "mind-index.yaml"), false)
	if err != nil || len(*doc.Value.Prompts) != 4 {
		t.Fatal(err)
	}
}

func TestProjectDuplicateDiagnosticUsesNodeKeyAndLine(t *testing.T) {
	root := canonicalPath(t.TempDir())
	putContent(t, root, "mind.yaml", "schema: '1'\n")
	raw := "schema: '1'\narticles:\n  first: {title: First, article_path: docs/first.md}\n# duplicate below\narticles:\n  second: {title: Second, article_path: docs/second.md}\n"
	putContent(t, root, "mind-index.yaml", raw)
	issues, _, err := LintProject(root, []string{"duplicate_key"}, false)
	if err != nil || len(issues) != 1 || issues[0].Kind != "duplicate_key" || issues[0].Message != "duplicate key 'articles' at line 5" {
		t.Fatalf("duplicate issue = %+v, %v", issues, err)
	}
	after, err := os.ReadFile(filepath.Join(root, "mind-index.yaml"))
	if err != nil || string(after) != raw {
		t.Fatalf("lint preview rewrote bytes: %q, %v", after, err)
	}
}
