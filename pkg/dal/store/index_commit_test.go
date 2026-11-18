package store

import (
	"errors"
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/require"
)

func TestIndexCommitRetainsCommittedAndOriginalWriterError(t *testing.T) {
	sentinel := errors.New("synthetic writer failure")
	for _, mode := range []string{"success", "equal-error", "different-error", "missing-error"} {
		t.Run(mode, func(t *testing.T) {
			path := filepath.Join(t.TempDir(), "mind-index.yaml")
			raw := []byte("schema: '1'\n")
			writer := func(name string, body []byte) error {
				switch mode {
				case "success":
					return os.WriteFile(name, body, 0644)
				case "equal-error":
					require.NoError(t, os.WriteFile(name, body, 0644))
				case "different-error":
					require.NoError(t, os.WriteFile(name, []byte("other"), 0644))
				}
				return sentinel
			}
			committed, err := CommitIndex(path, func() ([]byte, error) { return raw, nil }, writer)
			require.Equal(t, mode == "success" || mode == "equal-error", committed)
			if mode == "success" {
				require.NoError(t, err)
			} else {
				require.ErrorIs(t, err, sentinel)
			}
		})
	}
	called := false
	committed, err := CommitIndex("unused", func() ([]byte, error) { return nil, sentinel }, func(string, []byte) error { called = true; return nil })
	require.False(t, committed)
	require.ErrorIs(t, err, sentinel)
	require.False(t, called)
	path := filepath.Join(t.TempDir(), "mind-index.yaml")
	committed, err = CommitIndex(path, func() ([]byte, error) { return []byte("schema: '1'\n"), nil }, nil)
	require.True(t, committed)
	require.NoError(t, err)
}
