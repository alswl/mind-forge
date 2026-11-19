package source

import (
	"os"
	"testing"

	"github.com/stretchr/testify/require"
)

func TestSourceNewPreviewRetainsPartialValidationAndNameOverride(t *testing.T) {
	dir := t.TempDir()
	for _, tc := range []struct{ input, want string }{{dir + "/missing.md", "missing"}, {"/", "unknown"}, {".", "unknown"}} {
		result := PreviewNew(tc.input, nil)
		require.Equal(t, tc.want, result.Name)
		require.Equal(t, tc.input, result.Input)
		name := "explicit"
		require.Equal(t, name, PreviewNew(tc.input, &name).Name)
	}
	entries, err := os.ReadDir(dir)
	require.NoError(t, err)
	require.Empty(t, entries)
}
