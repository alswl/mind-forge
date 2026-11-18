package lifecycle

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/require"
)

func TestCancelledPreparationCanRecoverWithoutAuthoredChanges(t *testing.T) {
	root, plan := transactionFixture(t)
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	engine := Engine{Context: ctx, Checkpoint: func(stage string) error {
		if stage == "prepare:backup" {
			cancel()
		}
		return nil
	}}
	_, err := engine.Apply(root, plan)
	require.ErrorIs(t, err, context.Canceled)
	raw, err := os.ReadFile(filepath.Join(root, "a.md"))
	require.NoError(t, err)
	require.Equal(t, "before", string(raw))
	aborted, err := (Engine{}).RecoverUnpublished(root, false)
	require.NoError(t, err)
	if !aborted {
		journal, err := (Engine{}).ReadJournal(root)
		require.NoError(t, err)
		require.NotNil(t, journal)
		_, err = (Engine{}).Recover(root, false)
		require.NoError(t, err)
	}
	_, err = (Engine{}).Apply(root, plan)
	require.NoError(t, err)
	raw, err = os.ReadFile(filepath.Join(root, "a.md"))
	require.NoError(t, err)
	require.Equal(t, "after", string(raw))
}
