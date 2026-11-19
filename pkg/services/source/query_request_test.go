package source

import (
	"os"
	"path/filepath"
	"testing"

	outcome "github.com/alswl/mind-forge/pkg/common/outcome"
	"github.com/stretchr/testify/require"
)

func TestSourceQueryFiltersBeforeSelectorAndPreservesReadOnlyErrors(t *testing.T) {
	root, err := filepath.EvalSymlinks(sourceProject(t))
	require.NoError(t, err)
	putSource(t, root, "mind-index.yaml", "schema: '1'\nsources:\n first: {name: first, type: file, path: sources/a.md}\n second: {name: sources/a.md, type: web, url: 'https://example.invalid'}\n")
	before, err := os.ReadFile(filepath.Join(root, "mind-index.yaml"))
	require.NoError(t, err)
	filter := "first"
	result, err := ShowRequest(root, "sources/a.md", &filter, nil)
	require.NoError(t, err)
	require.Equal(t, "first", result.Selected.Name)
	require.Len(t, result.Items, 1)
	fileKind := "file"
	typedResult, typedErr := ShowRequest(root, "sources/a.md", nil, &fileKind)
	require.NoError(t, typedErr)
	require.Equal(t, "first", typedResult.Selected.Name)
	filter = "missing"
	_, err = ShowRequest(root, "first", &filter, nil)
	var typed *outcome.Error
	require.ErrorAs(t, err, &typed)
	require.Equal(t, "not_found", typed.Kind)
	_, err = ShowRequest(root, "sources/a.md", nil, nil)
	require.ErrorAs(t, err, &typed)
	require.Equal(t, "usage", typed.Kind)
	kind := "auto"
	_, err = ShowRequest(root, "../outside", nil, &kind)
	require.ErrorContains(t, err, "--type auto")
	_, err = ResolveUpdateRequest(root, "../outside")
	require.ErrorContains(t, err, "contains '..', which is not accepted in a project selector")
	name, err := ResolveUpdateRequest(root, "first")
	require.NoError(t, err)
	require.Equal(t, "first", name)
	after, err := os.ReadFile(filepath.Join(root, "mind-index.yaml"))
	require.NoError(t, err)
	require.Equal(t, before, after)
	putSource(t, root, "mind-index.yaml", "schema: [broken\n")
	_, err = ShowRequest(root, "../outside", nil, nil)
	require.Error(t, err)
	require.NotContains(t, err.Error(), "contains '..'")
	_, err = ResolveUpdateRequest(root, "../outside")
	require.ErrorContains(t, err, "contains '..'")
}
func TestSourceInputKindCompatibility(t *testing.T) {
	kind, err := InputKind("https://example.invalid", nil, nil)
	require.NoError(t, err)
	require.Nil(t, kind)
	auto := "auto"
	web := "web"
	file := "file"
	kind, err = InputKind("https://example.invalid", &auto, &web)
	require.NoError(t, err)
	require.Equal(t, "web", *kind)
	_, err = InputKind("https://example.invalid", &file, nil)
	require.ErrorContains(t, err, "cannot use --type pdf or --type file with a URL input")
	_, err = InputKind("missing.md", &web, nil)
	require.ErrorContains(t, err, "cannot use --type rss or --type web with a local file input")
}
