package main

import (
	"bytes"
	"io"
	"strings"
	"testing"

	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/spf13/cobra"
	"github.com/stretchr/testify/require"
)

func TestCommandFactoriesIsolateFlagAndChildState(t *testing.T) {
	first, err := NewCommand(nil)
	require.NoError(t, err)
	second, err := NewCommand(nil)
	require.NoError(t, err)
	one, _, err := first.Find([]string{"article", "list"})
	require.NoError(t, err)
	two, _, err := second.Find([]string{"article", "list"})
	require.NoError(t, err)
	require.NotSame(t, one, two)
	require.NoError(t, one.Flags().Set("no-headers", "__present__"))
	require.True(t, flagPresent(one, "no-headers"))
	require.False(t, flagPresent(two, "no-headers"))
	one.AddCommand(&cobra.Command{Use: "private-test-child"})
	require.Empty(t, two.Commands())
}

func TestEveryCatalogueCommandHasOwnFactoryAndParent(t *testing.T) {
	spec, err := LoadSpec()
	require.NoError(t, err)
	root, err := NewCommand(nil)
	require.NoError(t, err)
	for _, item := range spec.Commands {
		command, remaining, err := root.Find(item.Path)
		require.NoError(t, err)
		require.Empty(t, remaining)
		if len(item.Path) > 0 {
			require.Equal(t, item.Path[len(item.Path)-1], command.Name())
		}
	}
}

func TestRegisteredGroupsKeepHelpActions(t *testing.T) {
	root, err := NewCommand(nil)
	require.NoError(t, err)
	for _, path := range [][]string{{"publish"}, {"publish", "target"}, {"render", "template"}} {
		command, _, err := root.Find(path)
		require.NoError(t, err)
		var stdout bytes.Buffer
		command.SetOut(&stdout)
		action := commandRegistrations[strings.Join(path, " ")].run
		require.NoError(t, action(commandExecution{Command: command, Application: &app.Context{}, Stdout: &stdout, Stderr: io.Discard}))
		require.Contains(t, stdout.String(), "Usage:")
	}
}
