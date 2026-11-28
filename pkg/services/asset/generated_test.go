package asset

import (
	"errors"
	"os"
	"testing"

	"github.com/stretchr/testify/require"
)

func TestGeneratedPublicationPreservesOwnerAndRollsBack(t *testing.T) {
	dir := t.TempDir()
	root, err := os.OpenRoot(dir)
	require.NoError(t, err)
	defer root.Close()
	path := "media/reports/weekly/overview.svg"
	s := Service{}
	require.NoError(t, s.PublishGenerated(root, path, "weekly", []byte("first")))
	require.NoError(t, s.PublishGenerated(root, "media/reports/monthly/overview.svg", "monthly", []byte("monthly")))
	index, err := root.ReadFile("mind-index.yaml")
	require.NoError(t, err)
	require.Error(t, s.PublishGenerated(root, path, "another", []byte("other")))
	broken := Service{WriteIndex: func(string, []byte) error { return errors.New("synthetic failure") }}
	require.Error(t, broken.PublishGenerated(root, path, "weekly", []byte("second")))
	actual, err := root.ReadFile(path)
	require.NoError(t, err)
	require.Equal(t, "first", string(actual))
	actual, err = root.ReadFile("mind-index.yaml")
	require.NoError(t, err)
	require.Equal(t, index, actual)
	require.NoError(t, s.PublishGenerated(root, path, "weekly", []byte("second")))
	require.NoError(t, root.Symlink("weekly", "media/reports/alias"))
	require.Error(t, s.PublishGenerated(root, "media/reports/alias/overview.svg", "weekly", []byte("other")))
}
