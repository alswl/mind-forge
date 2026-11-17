package content

import (
	"os"
	"path/filepath"
	"testing"
)

func TestArticleLintChecksPrivacyAndFixesOnlyUnoccupiedNames(t *testing.T) {
	root := canonicalPath(t.TempDir())
	putContent(t, root, "mind.yaml", "schema: '1'\n")
	putContent(t, root, "docs/Bad_Name.md", "---\nmind-forge-visibility: private\n---\n# Title\n")
	putContent(t, root, "docs/empty.md", " \n")
	putContent(t, root, "docs/invalid.md", "---\nmind-forge-visibility: hidden\n---\n# Title\n")
	putContent(t, root, "docs/directory/01-opening.md", "---\nmind-forge-visibility: private\n---\n# not scanned\n")
	issues, err := LintArticles(root, false)
	if err != nil {
		t.Fatal(err)
	}
	kinds := map[string]int{}
	for _, issue := range issues {
		kinds[issue.Kind]++
	}
	for _, kind := range []string{"filename_convention", "empty_file", "mind_forge_private_title_block", "mind_forge_visibility_invalid"} {
		if kinds[kind] != 1 {
			t.Fatalf("issues %#v", issues)
		}
	}
	if _, err := LintArticles(root, true); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(filepath.Join(root, "docs/bad-name.md")); err != nil {
		t.Fatal("filename fix absent", err)
	}
	putContent(t, root, "docs/Bad_Name.md", "new author")
	if _, err := LintArticles(root, true); err != nil {
		t.Fatal(err)
	}
	if raw, _ := os.ReadFile(filepath.Join(root, "docs/Bad_Name.md")); string(raw) != "new author" {
		t.Fatal("occupied fix target overwritten")
	}
}
