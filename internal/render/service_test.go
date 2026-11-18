package render

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func renderFixture(t *testing.T, root, name, text string) {
	t.Helper()
	path := filepath.Join(root, name)
	if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(text), 0644); err != nil {
		t.Fatal(err)
	}
}

func TestTemplateDiscoveryAndFrontmatterPreserveGuidance(t *testing.T) {
	root := t.TempDir()
	renderFixture(t, root, ".mind-forge/renders/team.md", "---\nlabel: \"Team review\"\ndescription: Internal\n---\n\nKeep **meaning**.\n")
	items, err := Templates(root)
	if err != nil || len(items) != 3 {
		t.Fatalf("templates: %+v %v", items, err)
	}
	custom := items[2]
	if custom.Info.Name != "team" || custom.Info.Label != "Team review" || custom.Info.Description != "Internal" || custom.Body != "Keep **meaning**.\n" {
		t.Fatalf("template: %+v", custom)
	}
	renderFixture(t, root, ".mind-forge/renders/report.md", "Override")
	if _, err := Templates(root); err == nil {
		t.Fatal("accepted built-in collision")
	}
}

func TestPromptGenerationReadsBuiltOutputsAndWritesNothing(t *testing.T) {
	root := t.TempDir()
	renderFixture(t, root, "outputs/b.md", "# B\n")
	renderFixture(t, root, "outputs/a.md", "A")
	renderFixture(t, root, "outputs/empty.md", " \n")
	items, err := Templates(root)
	if err != nil {
		t.Fatal(err)
	}
	result, err := Generate(root, "project", "outputs", "md", nil, "report", "fragment", items)
	if err != nil || result.Scope != "project" || len(result.Outputs) != 2 || result.Outputs[0].Path != "outputs/a.md" {
		t.Fatalf("result: %+v %v", result, err)
	}
	if !strings.Contains(result.Prompt, "HTML fragment") || !strings.Contains(result.Prompt, "--- BEGIN MF OUTPUT: outputs/a.md ---\nA\n--- END") {
		t.Fatal(result.Prompt)
	}
	if strings.Index(result.Prompt, "outputs/a.md") > strings.Index(result.Prompt, "outputs/b.md") {
		t.Fatal("unstable order")
	}
	name := "empty"
	if _, err := Generate(root, "project", "outputs", "md", &name, "report", "", items); err == nil {
		t.Fatal("accepted blank output")
	}
	name = "b"
	if _, err := Generate(root, "project", "outputs", "md", &name, "REPORT", "", items); err == nil {
		t.Fatal("generation unexpectedly case-insensitive")
	}
	entries, err := os.ReadDir(root)
	if err != nil || len(entries) != 1 {
		t.Fatal("generated an artifact", entries, err)
	}
}

func TestTemplateParserMalformedFrontmatterIsLiteral(t *testing.T) {
	raw := "---\nlabel: Incomplete\nBody"
	label, desc, body := ParseTemplate(raw, "fallback")
	if label != "fallback" || desc != "" || body != raw {
		t.Fatalf("%q %q %q", label, desc, body)
	}
}
