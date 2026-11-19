package config

import (
	"errors"
	"os"
	"path/filepath"
	"testing"

	output "github.com/alswl/mind-forge/pkg/common/outcome"
	"github.com/stretchr/testify/require"
)

func TestRuntimeFailuresPreserveCauseAndUsageWire(t *testing.T) {
	sentinel := errors.New("synthetic directory lookup failure")
	for _, kind := range []string{"directory", "missing", "toml"} {
		t.Run(kind, func(t *testing.T) {
			in := runtimeFixture(t, "")
			var want error
			var message string
			switch kind {
			case "directory":
				in.UserConfigDir = func() (string, error) { return "", sentinel }
				want, message = sentinel, "runtime config directory: "+sentinel.Error()
			case "missing":
				path := filepath.Join(in.CWD, "missing.toml")
				in.CLI["config"] = path
				_, want = os.ReadFile(path)
				message = "read runtime config " + path + ": " + want.Error()
			case "toml":
				in = runtimeFixture(t, "root = [")
			}
			_, err := ResolveRuntime(in)
			var typed *output.Error
			require.ErrorAs(t, err, &typed)
			require.NotNil(t, errors.Unwrap(err))
			if kind == "missing" {
				require.ErrorIs(t, err, os.ErrNotExist)
			}
			if kind == "directory" {
				require.ErrorIs(t, err, want)
			}
			if message != "" {
				require.Equal(t, message, err.Error())
			}
			require.Equal(t, 2, typed.ExitCode())
			require.Equal(t, output.Usage(typed.Message, "").JSON(), typed.JSON())
		})
	}
}
