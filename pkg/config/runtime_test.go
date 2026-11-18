package config

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/require"
)

func runtimeFixture(t *testing.T, body string) RuntimeInput {
	t.Helper()
	cwd := t.TempDir()
	dir := t.TempDir()
	require.NoError(t, os.Mkdir(filepath.Join(dir, "mf"), 0755))
	require.NoError(t, os.WriteFile(filepath.Join(dir, "mf/config.toml"), []byte(body), 0644))
	return RuntimeInput{CWD: cwd, Env: map[string]string{}, CLI: map[string]string{}, UserConfigDir: func() (string, error) { return dir, nil }}
}

func TestRuntimePrecedencePresenceAndIsolation(t *testing.T) {
	in := runtimeFixture(t, "quiet=true\nverbose=0\nno-color=true\noutput='json'\nroot='repo'\nproject='lab'\n")
	in.Env = map[string]string{"MF_QUIET": "garbage-shadowed", "MF_VERBOSE": "19", "MF_ROOT": "env-root"}
	in.CLI = map[string]string{"quiet": "false", "verbose": "0", "no-color": "false", "root": "", "project": ""}
	s, err := ResolveRuntime(in)
	require.NoError(t, err)
	require.False(t, s.Quiet)
	require.False(t, s.NoColor)
	require.Zero(t, s.Verbose)
	require.Empty(t, s.Root)
	require.Empty(t, s.Project)
	require.Equal(t, "json", s.Output)
	for _, k := range []string{"quiet", "verbose", "no-color", "root", "project"} {
		require.True(t, s.Present[k])
		require.Equal(t, "cli", s.Sources[k])
	}
	in.CLI = map[string]string{"quiet": "false"}
	in.Env = map[string]string{"MF_VERBOSE": "2"}
	next, err := ResolveRuntime(in)
	require.NoError(t, err)
	require.Equal(t, uint(2), next.Verbose)
	require.Equal(t, "env", next.Sources["verbose"])
	require.Equal(t, "file", next.Sources["root"])
	require.Equal(t, filepath.Join(filepath.Dir(next.Config), "repo"), next.Root)
	require.Equal(t, "lab", next.Project)
	require.Zero(t, s.Verbose)
	require.Empty(t, s.Root)
}

func TestRuntimeStrictFilesAndSelectedEnvironment(t *testing.T) {
	for _, body := range []string{"quiet='false'", "verbose='1'", "verbose=-1", "verbose=1.2", "root=1", "output=''", "config='x'", "[unknown]", "[quiet]", "[root]\nchild='x'", "unknown=1", "output='xml'"} {
		t.Run(body, func(t *testing.T) { in := runtimeFixture(t, body); _, err := ResolveRuntime(in); require.Error(t, err) })
	}
	for _, key := range []string{"output", "quiet", "verbose", "no-color"} {
		t.Run(key, func(t *testing.T) {
			in := runtimeFixture(t, "")
			in.Env[envKey(key)] = ""
			_, err := ResolveRuntime(in)
			require.Error(t, err)
		})
	}
	for _, value := range []string{"true", "TRUE", "1", "false", "FALSE", "0"} {
		in := runtimeFixture(t, "")
		in.Env["MF_QUIET"] = value
		_, err := ResolveRuntime(in)
		require.NoError(t, err)
	}
	in := runtimeFixture(t, "")
	in.Env["MF_VERBOSE"] = "+1"
	_, err := ResolveRuntime(in)
	require.Error(t, err)
	in.Env["MF_VERBOSE"] = "1"
	in.Env["MF_QUIET"] = "true"
	_, err = ResolveRuntime(in)
	require.Error(t, err)
}

func TestRuntimeBootstrapPathsAndErrorFormat(t *testing.T) {
	in := runtimeFixture(t, "")
	in.CLI = map[string]string{"config": "local.toml"}
	in.Env["MF_CONFIG"] = "shadowed"
	in.UserConfigDir = func() (string, error) { t.Fatal("explicit config queried default directory"); return "", nil }
	require.NoError(t, os.WriteFile(filepath.Join(in.CWD, "local.toml"), []byte("root='child'\noutput='json'"), 0644))
	s, err := ResolveRuntime(in)
	require.NoError(t, err)
	require.Equal(t, filepath.Join(in.CWD, "child"), s.Root)
	require.Equal(t, "cli", s.Sources["config"])
	in.CLI["config"] = "absent"
	in.Env["MF_OUTPUT"] = "json"
	s, err = ResolveRuntime(in)
	require.Error(t, err)
	require.Equal(t, "json", s.Output)
	in.CLI["config"] = ""
	_, err = ResolveRuntime(in)
	require.Error(t, err)
	delete(in.CLI, "config")
	delete(in.Env, "MF_CONFIG")
	in.UserConfigDir = func() (string, error) { return t.TempDir(), nil }
	_, err = ResolveRuntime(in)
	require.NoError(t, err)
}

func TestRuntimeFreezesInputAndUsesKnownErrorEnvelope(t *testing.T) {
	in := runtimeFixture(t, "output='json'\n")
	in.Env["MF_QUIET"] = "false"
	dir, _ := in.UserConfigDir()
	in.UserConfigDir = func() (string, error) { in.Env["MF_QUIET"] = "true"; return dir, nil }
	s, err := ResolveRuntime(in)
	require.NoError(t, err)
	require.False(t, s.Quiet)
	in.Env["MF_OUTPUT"] = "invalid"
	s, err = ResolveRuntime(in)
	require.Error(t, err)
	require.Equal(t, "json", s.Output)
	in = runtimeFixture(t, "output='json'\n[unknown]\n")
	s, err = ResolveRuntime(in)
	require.Error(t, err)
	require.Equal(t, "json", s.Output)
}
