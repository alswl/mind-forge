package main

import (
	"bytes"
	"encoding/json"
	"testing"

	"github.com/stretchr/testify/require"
)

func TestUsageErrorsHonorJSONFlagBeforeParsingCompletes(t *testing.T) {
	for _, test := range []struct {
		name string
		args []string
		want string
	}{
		{"before unknown command", []string{"--json", "unknown-command"}, "unknown command"},
		{"after unknown command", []string{"unknown-command", "--json"}, "unknown command"},
		{"separate output flag", []string{"unknown-command", "-o", "json"}, "unknown command"},
		{"bound flag before parser error", []string{"--json", "build", "--badflag"}, "unknown flag"},
	} {
		t.Run(test.name, func(t *testing.T) {
			var stdout, stderr bytes.Buffer
			require.Equal(t, 2, Run(t.Context(), test.args, &stdout, &stderr))
			require.Empty(t, stdout.String())
			var body struct {
				Status  string
				Command string
				Error   struct{ Kind, Message string }
			}
			require.NoError(t, json.Unmarshal(stderr.Bytes(), &body))
			require.Equal(t, "error", body.Status)
			require.Equal(t, "mf-report", body.Command)
			require.Equal(t, "usage", body.Error.Kind)
			require.Contains(t, body.Error.Message, test.want)
		})
	}
}

func TestRegisteredCommandsUseFreshFactoriesAndShowBuildExample(t *testing.T) {
	first, second := NewCommand(), NewCommand()
	for _, path := range [][]string{{"build"}, {"list"}, {"openapi"}, {"serve"}, {"catalog"}, {"catalog", "build"}} {
		one, rest, err := first.Find(path)
		require.NoError(t, err)
		require.Empty(t, rest)
		two, rest, err := second.Find(path)
		require.NoError(t, err)
		require.Empty(t, rest)
		require.Equal(t, path[len(path)-1], one.Name())
		require.NotSame(t, one, two)
	}
	var stdout, stderr bytes.Buffer
	require.Zero(t, Run(t.Context(), []string{"build", "--help"}, &stdout, &stderr))
	require.Contains(t, stdout.String(), "mf-report --workdir ./weekly build --view table")
}
