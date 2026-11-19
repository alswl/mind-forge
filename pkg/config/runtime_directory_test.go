package config

import (
	"github.com/stretchr/testify/require"
	"testing"
)

func TestSnapshotDirectoryMatchesStandardRulesAndErrors(t *testing.T) {
	for _, item := range []struct {
		platform      string
		env           map[string]string
		want, message string
	}{
		{"darwin", map[string]string{"HOME": "/synthetic"}, "/synthetic/Library/Application Support", ""},
		{"ios", map[string]string{}, "", "$HOME is not defined"},
		{"linux", map[string]string{"HOME": "/synthetic"}, "/synthetic/.config", ""},
		{"linux", map[string]string{"XDG_CONFIG_HOME": "/settings"}, "/settings", ""},
		{"linux", map[string]string{"XDG_CONFIG_HOME": "relative"}, "", "path in $XDG_CONFIG_HOME is relative"},
		{"linux", nil, "", "neither $XDG_CONFIG_HOME nor $HOME are defined"},
		{"windows", map[string]string{"AppData": "synthetic-app-data"}, "synthetic-app-data", ""},
		{"windows", nil, "", "%AppData% is not defined"},
		{"plan9", map[string]string{"home": "/synthetic"}, "/synthetic/lib", ""},
		{"plan9", nil, "", "$home is not defined"},
	} {
		got, err := snapshotUserConfigDir(item.platform, item.env)
		require.Equal(t, item.want, got)
		if item.message == "" {
			require.NoError(t, err)
		} else {
			require.EqualError(t, err, item.message)
		}
	}
}

func TestExplicitRuntimeFileSkipsDefaultLookup(t *testing.T) {
	in := runtimeFixture(t, "output='json'\n")
	first, err := ResolveRuntime(in)
	require.NoError(t, err)
	in.CLI["config"] = first.Config
	in.Env = nil
	in.UserConfigDir = func() (string, error) { t.Fatal("explicit config queried default directory"); return "", nil }
	got, err := ResolveRuntime(in)
	require.NoError(t, err)
	require.Equal(t, "json", got.Output)
}
