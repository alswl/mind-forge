package build

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
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
