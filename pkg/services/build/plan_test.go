package build

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/alswl/mind-forge/pkg/common/outcome"
	"github.com/stretchr/testify/require"
)

func buildTestRoot(t *testing.T) string {
	t.Helper()
	root, err := filepath.EvalSymlinks(t.TempDir())
	if err != nil {
		t.Fatal(err)
	}
	return root
}

func putBuildFile(t *testing.T, root, name, body string) string {
	t.Helper()
	path := filepath.Join(root, name)
	if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(body), 0644); err != nil {
		t.Fatal(err)
	}
	return path
}

func TestBuildPlanIsReadOnlyAndUsesLexicalSourceOrder(t *testing.T) {
	root := buildTestRoot(t)
	putBuildFile(t, root, "mind.yaml", "schema: '1'\nbuild:\n  strip_first_h1: true\n  banner: {text: Notice, level: note}\n  merge_order: [declared-order]\n")
	putBuildFile(t, root, "mind-index.yaml", "schema: '1'\narticles:\n- title: Display only\n  article_path: docs/post/\n")
	putBuildFile(t, root, "docs/post/20-body.md", "## Body\n")
	putBuildFile(t, root, "docs/post/10-title.md", "# Title\n")
	p, err := Prepare(root, root, "post", Options{DryRun: true})
	if err != nil {
		t.Fatal(err)
	}
	if len(p.Plan.InputFiles) != 2 || p.Plan.InputFiles[0].Path != "docs/post/10-title.md" || p.Plan.MergeOrder[0] != "declared-order" || p.Plan.OutputPath != filepath.Join(root, "outputs/post.md") || p.Plan.Banner == nil {
		t.Fatalf("plan: %+v", p.Plan)
	}
	if _, err := os.Stat(filepath.Join(root, "outputs")); !os.IsNotExist(err) {
		t.Fatal("planning created output directory")
	}
	body, _, err := p.Render()
	if err != nil || strings.Contains(body, "# Title") || !strings.Contains(body, ":::note\nNotice\n:::") || !strings.Contains(body, "## Body") {
		t.Fatalf("render: %q %v", body, err)
	}
}

func TestBuildSelectedOutputStaysUnderBuildAndKeepsPrivateOut(t *testing.T) {
	for _, layout := range []string{"category", "bundle"} {
		t.Run(layout, func(t *testing.T) {
			root := buildTestRoot(t)
			putBuildFile(t, root, "mind.yaml", "schema: '1'\ncontent_layout: "+layout+"\nminds: {idea: {default_output: summary}}\n")
			path := "outputs/idea/summary.md"
			if layout == "bundle" {
				path = "idea/outputs/summary.md"
			}
			original := "# Public\n\n> [!mf-private]\n> SECRET\n\nPublic.\n"
			putBuildFile(t, root, path, original)
			p, err := Prepare(root, root, "::idea", Options{Format: "html"})
			if err != nil {
				t.Fatal(err)
			}
			if p.Location == nil || p.Location.Path != path || p.Plan.OutputPath != filepath.Join(root, "build/idea/summary.html") {
				t.Fatalf("location: %+v", p)
			}
			body, _, err := p.Render()
			if err != nil || strings.Contains(body, "SECRET") || !strings.Contains(body, "<h1>Public</h1>") {
				t.Fatalf("HTML: %s %v", body, err)
			}
			out := filepath.Join(root, path)
			if _, err := Prepare(root, root, "::idea", Options{Out: &out}); err == nil {
				t.Fatal("source overlap accepted")
			}
			outside := filepath.Join(t.TempDir(), "summary.md")
			if _, err := Prepare(root, root, "::idea", Options{Out: &outside}); err == nil {
				t.Fatal("mind output escaped build")
			}
			after, _ := os.ReadFile(filepath.Join(root, path))
			if string(after) != original {
				t.Fatal("render edited source")
			}
		})
	}
}

func TestBuildConfiguredDirectoryAndTemplateValidation(t *testing.T) {
	root := buildTestRoot(t)
	putBuildFile(t, root, "mind.yaml", "schema: '1'\nbuild:\n  articles:\n    post: {article_dir: authored}\n  html_template: templates/page.html\n")
	putBuildFile(t, root, "authored/title.md", "# Public\n")
	putBuildFile(t, root, "templates/page.html", "<main>No content slot</main>")
	if _, err := Prepare(root, root, "post", Options{Format: "html", DryRun: true}); err == nil {
		t.Fatal("invalid template accepted in preview")
	}
	putBuildFile(t, root, "templates/page.html", "<main>{{content}}</main>")
	p, err := Prepare(root, root, "post", Options{})
	if err != nil || p.Plan.ArticlePath != filepath.Join(root, "authored") {
		t.Fatalf("declared path: %+v %v", p, err)
	}
	if _, err := os.Stat(filepath.Join(root, "outputs")); !os.IsNotExist(err) {
		t.Fatal("template failure created output")
	}
}

func TestBuildRebasesReferencesAndRefusesPrivateTitleBeforePublishing(t *testing.T) {
	root := buildTestRoot(t)
	putBuildFile(t, root, "mind.yaml", "schema: '1'\n")
	source := putBuildFile(t, root, "docs/post.md", "# Public\n\n![asset](../assets/a.png)\n")
	p, err := Prepare(root, root, "post", Options{Source: &source})
	if err != nil {
		t.Fatal(err)
	}
	body, _, err := p.Render()
	if err != nil || !strings.Contains(body, "../assets/a.png") {
		t.Fatalf("reference: %q %v", body, err)
	}
	putBuildFile(t, root, "docs/post.md", "---\nmind-forge-visibility: private\n---\n# Secret\n")
	if _, _, err := p.Render(); err == nil {
		t.Fatal("private title accepted")
	}
}

func TestLegacyBuildRefusesOutputOverlappingAuthorSource(t *testing.T) {
	root := buildTestRoot(t)
	putBuildFile(t, root, "mind.yaml", "schema: '1'\n")
	source := putBuildFile(t, root, "docs/post.md", "# Public\n")
	if _, err := Prepare(root, root, "post", Options{Source: &source, Out: &source}); err == nil {
		t.Fatal("accepted source file as artifact")
	}
	directory := filepath.Dir(source)
	out := filepath.Join(directory, "rendered.md")
	if _, err := Prepare(root, root, "post", Options{Source: &directory, Out: &out}); err == nil {
		t.Fatal("accepted artifact inside author directory")
	}
}

func TestLegacyBuildRejectsAbandonedArticleBySourceIdentity(t *testing.T) {
	root := buildTestRoot(t)
	putBuildFile(t, root, "mind.yaml", "schema: '1'\nbuild:\n  articles:\n    post: {article_dir: authored}\n")
	index := "schema: '1'\narticles:\n- title: Bundle\n  article_path: alias-bundle\n- title: Post\n  article_path: authored/post.md\n  status: "
	putBuildFile(t, root, "mind-index.yaml", index+"abandoned\n")
	path := putBuildFile(t, root, "authored/post.md", "# Synthetic post\n")
	alias := filepath.Join(root, "alias.md")
	require.NoError(t, os.Symlink(path, alias))
	hardLink := filepath.Join(root, "hard-link.md")
	require.NoError(t, os.Link(path, hardLink))
	other := putBuildFile(t, root, "other/post.md", "# Unregistered post\n")
	authored := filepath.Dir(path)
	aliasBundle := filepath.Join(root, "alias-bundle")
	require.NoError(t, os.Mkdir(aliasBundle, 0o755))
	require.NoError(t, os.Symlink(path, filepath.Join(aliasBundle, "opening.md")))
	for _, dryRun := range []bool{true, false} {
		for _, entry := range []struct {
			selector string
			source   *string
		}{
			{"authored/post", nil}, {"post", nil}, {"explicit", &path},
			{"explicit", &alias}, {"explicit", &hardLink}, {"explicit", &authored},
			{"explicit", &aliasBundle}, {"alias-bundle", nil},
		} {
			_, err := Prepare(root, root, entry.selector, Options{Source: entry.source, DryRun: dryRun})
			require.ErrorContains(t, err, "cannot build abandoned article")
			var diagnostic *outcome.Error
			require.ErrorAs(t, err, &diagnostic)
			require.Equal(t, "usage", diagnostic.Kind)
		}
	}
	require.NoDirExists(t, filepath.Join(root, "outputs"))
	_, err := Prepare(root, root, "explicit", Options{Source: &other})
	require.NoError(t, err)
	for _, status := range []string{"completed", "published", "draft"} {
		putBuildFile(t, root, "mind-index.yaml", index+status+"\n")
		_, err := Prepare(root, root, "post", Options{})
		require.NoError(t, err, status)
	}
}

func TestLegacyBuildPreservesNonIndexedSelectionWithBrokenIndex(t *testing.T) {
	root := buildTestRoot(t)
	putBuildFile(t, root, "mind.yaml", "schema: '1'\nbuild:\n  articles:\n    post: {article_dir: authored}\n")
	path := putBuildFile(t, root, "authored/post.md", "# Synthetic post\n")
	broken := "articles: ["
	index := putBuildFile(t, root, "mind-index.yaml", broken)
	for _, entry := range []struct {
		selector string
		source   *string
	}{{"post", nil}, {"explicit", &path}} {
		preview, err := Prepare(root, root, entry.selector, Options{Source: entry.source, DryRun: true})
		require.NoError(t, err)
		require.Len(t, preview.Plan.InputFiles, 1)
		require.NoFileExists(t, preview.Plan.OutputPath)
		prepared, err := Prepare(root, root, entry.selector, Options{Source: entry.source})
		require.NoError(t, err)
		_, warnings, err := prepared.Execute(root, nil)
		require.NoError(t, err)
		require.FileExists(t, preview.Plan.OutputPath)
		require.Len(t, warnings, 1)
		require.Contains(t, warnings[0], "auto-index failed: yaml parse error")
	}
	_, err := Prepare(root, root, "indexed", Options{DryRun: true})
	require.ErrorContains(t, err, "yaml parse error")
	raw, err := os.ReadFile(index)
	require.NoError(t, err)
	require.Equal(t, broken, string(raw))
}
