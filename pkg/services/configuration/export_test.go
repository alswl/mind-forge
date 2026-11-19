package configuration

import (
	"errors"
	"os"
	"testing"

	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/stretchr/testify/require"
)

func TestConfigExportPreservesBusinessSerializationAndWriterFailure(t *testing.T) {
	cwd := t.TempDir()
	sentinel := errors.New("synthetic export failure")
	config, err := app.EffectiveConfig(cwd, "")
	require.NoError(t, err)
	for _, format := range []string{"yaml", "json", "unknown"} {
		expected, err := app.SerializeConfig(config, format)
		require.NoError(t, err)
		called := false
		service := Exporter{Write: func(path string, raw []byte) error {
			called = true
			require.Equal(t, "explicit-out", path)
			require.Equal(t, expected, string(raw))
			return sentinel
		}}
		err = service.Export(cwd, "", format, "explicit-out")
		require.ErrorIs(t, err, sentinel)
		require.True(t, called)
	}
	entries, err := os.ReadDir(cwd)
	require.NoError(t, err)
	require.Empty(t, entries)
}
