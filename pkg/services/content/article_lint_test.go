package content

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/require"
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

func TestArticleLintStatusFiltersBeforeReadingOrFixing(t *testing.T) {
	root := canonicalPath(t.TempDir())
	putContent(t, root, "mind.yaml", "schema: '1'\nplugins:\n  typora-front-matter: {enabled: true}\n")
	putContent(t, root, "mind-index.yaml", "schema: '1'\narticles:\n- title: Retired\n  article_path: docs/Retired.md\n  status: abandoned\n- title: Finished\n  article_path: docs/finished/\n  status: completed\n")
	putContent(t, root, "docs/Retired.md", "\xff")
	putContent(t, root, "docs/finished/01-opening.md", "# Finished\n")
	putContent(t, root, "docs/Synthetic_Draft.md", "# Draft\n")

	draft := "draft"
	issues, err := LintArticlesByStatus(root, true, &draft)
	require.NoError(t, err)
	require.NotEmpty(t, issues)
	for _, issue := range issues {
		require.Equal(t, "docs/Synthetic_Draft.md", issue.Path)
	}
	require.FileExists(t, filepath.Join(root, "docs/synthetic-draft.md"))
	require.FileExists(t, filepath.Join(root, "docs/Retired.md"))

	completed := "completed"
	issues, err = LintArticlesByStatus(root, false, &completed)
	require.NoError(t, err)
	require.Len(t, issues, 1)
	require.Equal(t, "docs/finished/01-opening.md", issues[0].Path)
	require.Equal(t, "missing_typora_front_matter", issues[0].Kind)
}
