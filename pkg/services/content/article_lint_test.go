package content

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	"github.com/alswl/mind-forge/pkg/common/model"
	"github.com/alswl/mind-forge/pkg/dal/store"
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

func TestArticleLintFixRenamesIndexedArticleAndBindingsTogether(t *testing.T) {
	root := canonicalPath(t.TempDir())
	project := mutationProject(t, root, "synthetic")
	putContent(t, project, "mind.yaml", "schema: '1'\nplugins:\n  typora-front-matter: {enabled: false}\n")
	const oldPath, newPath = "docs/Retired_Post.md", "docs/retired-post.md"
	const rawIndex = "schema: '1'\narticles:\n- title: Retired\n  article_path: docs/Retired_Post.md\n  status: abandoned\n  created_at: original\n"
	const body = "# Retired\r\n"
	putContent(t, project, "mind-index.yaml", rawIndex)
	putContent(t, project, oldPath, body)
	putContent(t, project, "prompts/Retired_Post.md", "---\narticle: "+oldPath+"\n---\nPrompt\n")
	putContent(t, project, "prompts/review.md", "---\r\narticle: '"+oldPath+"'\r\n---\r\nReview\r\n")
	putContent(t, project, "thinking/Retired_Post.md", "Synthetic thought\n")
	putContent(t, project, "docs/Other_Post.md", "# Unregistered\n")

	abandoned, published := "abandoned", "published"
	lint := func(fix bool, status *string) ([]model.LintIssue, error) {
		return LintArticlesRequest(context.Background(), root, project, fix, status)
	}
	read := func(path string) string {
		data, err := os.ReadFile(filepath.Join(project, path))
		require.NoError(t, err)
		return string(data)
	}
	issues, err := lint(false, &abandoned)
	require.NoError(t, err)
	require.Len(t, issues, 1)
	require.Equal(t, oldPath, issues[0].Path)
	issues, err = lint(true, &published)
	require.NoError(t, err)
	require.Empty(t, issues)
	require.Equal(t, rawIndex, read("mind-index.yaml"))
	require.Equal(t, body, read(oldPath))

	putContent(t, project, "thinking/retired-post.md", "Occupied\n")
	_, err = lint(true, &abandoned)
	require.Error(t, err)
	require.Equal(t, rawIndex, read("mind-index.yaml"))
	require.Equal(t, body, read(oldPath))
	require.NoFileExists(t, filepath.Join(project, newPath))
	require.NoError(t, os.Remove(filepath.Join(project, "thinking/retired-post.md")))

	_, err = lint(true, &abandoned)
	require.NoError(t, err)
	require.NoFileExists(t, filepath.Join(project, oldPath))
	require.Equal(t, body, read(newPath))
	require.Equal(t, "---\narticle: "+newPath+"\n---\nPrompt\n", read("prompts/retired-post.md"))
	require.Equal(t, "---\r\narticle: "+newPath+"\r\n---\r\nReview\r\n", read("prompts/review.md"))
	require.Equal(t, "Synthetic thought\n", read("thinking/retired-post.md"))
	require.Equal(t, "# Unregistered\n", read("docs/Other_Post.md"))
	index, err := store.ReadIndex(filepath.Join(project, "mind-index.yaml"), false)
	require.NoError(t, err)
	require.Len(t, *index.Value.Articles, 1)
	require.Equal(t, newPath, (*index.Value.Articles)[0].Path)
	require.Equal(t, "abandoned", (*index.Value.Articles)[0].Status)
	require.Equal(t, "original", (*index.Value.Articles)[0].CreatedAt)
	require.Equal(t, newPath, (*index.Value.Thinking)[0].Article)
}
