package content

import (
	"encoding/json"
	"testing"

	"github.com/stretchr/testify/require"
)

func TestListOutputsRequestPreservesDiscoveryExpansionAndSelection(t *testing.T) {
	root := canonicalPath(t.TempDir())
	putContent(t, root, "mind.yaml", "schema_version: '1'\ncontent_layout: category\nminds: {empty: {}, alpha: {}, beta: {}}\n")
	for _, path := range []string{"outputs/alpha/a.md", "outputs/alpha/z.md", "outputs/beta/b.md"} {
		putContent(t, root, path, "# Synthetic\n")
	}
	outputs, err := ListOutputsRequest(root, root, nil, nil)
	require.NoError(t, err)
	paths := []string{}
	for _, item := range outputs {
		paths = append(paths, item.Path)
	}
	require.Equal(t, []string{"outputs/alpha/a.md", "outputs/alpha/z.md", "outputs/beta/b.md"}, paths)
	selector := "alpha"
	outputs, err = ListOutputsRequest(root, root, nil, &selector)
	require.NoError(t, err)
	require.Len(t, outputs, 2)
	selector = "empty"
	outputs, err = ListOutputsRequest(root, root, nil, &selector)
	require.NoError(t, err)
	raw, err := json.Marshal(outputs)
	require.NoError(t, err)
	require.JSONEq(t, "[]", string(raw))
	selector = "missing"
	_, err = ListOutputsRequest(root, root, nil, &selector)
	require.Error(t, err)
}
