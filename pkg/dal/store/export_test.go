package store

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/require"
)

func TestExportFilePreservesExistingModeLinksAndExplicitDestination(t *testing.T) {
	root := t.TempDir()
	outside := t.TempDir()
	target := filepath.Join(outside, "private.yaml")
	require.NoError(t, os.WriteFile(target, []byte("old"), 0600))
	before, err := os.Stat(target)
	require.NoError(t, err)
	link := filepath.Join(root, "config.yaml")
	require.NoError(t, os.Symlink(target, link))
	require.NoError(t, ExportFile(link, []byte("new")))
	after, err := os.Stat(target)
	require.NoError(t, err)
	require.True(t, os.SameFile(before, after))
	require.Equal(t, os.FileMode(0600), after.Mode().Perm())
	info, err := os.Lstat(link)
	require.NoError(t, err)
	require.NotZero(t, info.Mode()&os.ModeSymlink)
	raw, err := os.ReadFile(target)
	require.NoError(t, err)
	require.Equal(t, "new", string(raw))
	created := filepath.Join(outside, "new.yaml")
	reference := filepath.Join(outside, "reference.yaml")
	require.NoError(t, ExportFile(created, []byte("export")))
	require.NoError(t, os.WriteFile(reference, []byte("reference"), 0666))
	actual, err := os.Stat(created)
	require.NoError(t, err)
	expected, err := os.Stat(reference)
	require.NoError(t, err)
	require.Equal(t, expected.Mode().Perm(), actual.Mode().Perm())
	require.Error(t, ExportFile(outside, []byte("error")))
}
