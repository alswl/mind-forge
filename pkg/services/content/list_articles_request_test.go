package content

import (
	"errors"
	"os"
	"path/filepath"
	"testing"

	"github.com/alswl/mind-forge/pkg/common/model"
	"github.com/stretchr/testify/require"
)

func TestListArticlesRequestPreservesGlobalErrorsSortingAndWarningOrder(t *testing.T) {
	root := canonicalPath(t.TempDir())
	sentinel := errors.New("synthetic load fault")
	calls := []string{}
	roots := func(string) ([]string, error) { return []string{"first", "broken", "last"}, nil }
	load := func(project string) ([]ArticleListItem, []string, error) {
		calls = append(calls, "read:"+project)
		if project == "broken" {
			return []ArticleListItem{{Article: model.Article{Path: "discard"}}}, []string{project}, sentinel
		}
		if project == "first" {
			return []ArticleListItem{{Article: model.Article{Path: "z"}, Mtime: 7}, {Article: model.Article{Path: "equal"}, Mtime: 5, Project: "first"}}, []string{project}, nil
		}
		return []ArticleListItem{{Article: model.Article{Path: "a"}, Mtime: 7}, {Article: model.Article{Path: "equal"}, Mtime: 5, Project: "last"}}, []string{project}, nil
	}
	diagnostic := func(warnings []string) error { calls = append(calls, "warn:"+warnings[0]); return nil }
	result, err := listArticlesRequest(root, root, nil, diagnostic, roots, load)
	require.NoError(t, err)
	require.True(t, result.All)
	require.Empty(t, result.Project)
	require.Equal(t, []string{"read:first", "warn:first", "read:broken", "warn:broken", "read:last", "warn:last"}, calls)
	paths := []string{}
	for _, item := range result.Items {
		paths = append(paths, item.Article.Path)
	}
	require.Equal(t, []string{"a", "z", "equal", "equal"}, paths)
	require.Equal(t, "first", result.Items[2].Project)
	require.Equal(t, "last", result.Items[3].Project)
	calls = nil
	_, err = listArticlesRequest(root, root, nil, func([]string) error { return sentinel }, roots, load)
	require.ErrorIs(t, err, sentinel)
	require.Equal(t, []string{"read:first"}, calls)
}
func TestListArticlesRequestPreservesSingleProjectErrorPrecedence(t *testing.T) {
	root := canonicalPath(t.TempDir())
	putContent(t, root, "mind.yaml", "schema: '1'\n")
	sentinel := errors.New("synthetic query fault")
	warned := false
	roots := func(string) ([]string, error) { t.Fatal("single project enumerated roots"); return nil, nil }
	load := func(string) ([]ArticleListItem, []string, error) { return nil, []string{"warning"}, sentinel }
	_, err := listArticlesRequest(root, root, nil, func([]string) error { warned = true; return nil }, roots, load)
	require.ErrorIs(t, err, sentinel)
	require.False(t, warned)
	selected := "absent"
	_, err = listArticlesRequest(root, root, &selected, nil, roots, load)
	require.Error(t, err)
	require.NotErrorIs(t, err, sentinel)
}
func TestListArticlesRequestRetainsReadOnlyIndexViews(t *testing.T) {
	root := canonicalPath(t.TempDir())
	putContent(t, root, "mind.yaml", "schema: '1'\n")
	putContent(t, root, "mind-index.yaml", "schema: '1'\narticles: []\n")
	before, err := os.ReadFile(filepath.Join(root, "mind-index.yaml"))
	require.NoError(t, err)
	result, err := ListArticlesRequest(root, root, nil, nil)
	require.NoError(t, err)
	require.False(t, result.All)
	require.Equal(t, root, result.Project)
	require.Empty(t, result.Items)
	after, err := os.ReadFile(filepath.Join(root, "mind-index.yaml"))
	require.NoError(t, err)
	require.Equal(t, before, after)
}
