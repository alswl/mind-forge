package main

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/stretchr/testify/require"
)

func TestRuntimeFlagResetsAndCountStartAtZero(t *testing.T) {
	for _, args := range [][]string{{"--quiet=false", "--no-color=0", "--verbose=0"}, {"--quiet=FALSE", "-vv"}} {
		command := parserCommand(t, []string{"article", "list"})
		require.NoError(t, command.ParseFlags(args))
		require.NoError(t, validateCommand(command, nil))
		in := runtimeInput(command, "/synthetic", map[string]string{"MF_VERBOSE": "99", "MF_QUIET": "true"})
		require.Equal(t, "false", in.CLI["quiet"])
		require.NotEqual(t, "99", in.CLI["verbose"])
	}
	command := parserCommand(t, []string{"article", "list"})
	require.NoError(t, command.ParseFlags([]string{"--quiet=false", "--verbose=0"}))
	require.True(t, flagPresent(command, "quiet"))
	require.True(t, flagPresent(command, "verbose"))
	for _, arg := range []string{"--quiet=bad", "--verbose=-1", "--verbose=", "--verbose=+1", "--dry-run=false"} {
		c := parserCommand(t, []string{"source", "new"})
		require.Error(t, c.ParseFlags([]string{arg}))
	}
}

func TestRuntimeEffectiveProjectBlocksInferenceAndEmptyAllowsIt(t *testing.T) {
	a := &app.Context{RepoRoot: t.TempDir(), CWD: t.TempDir()}
	for _, source := range []string{"cli", "env", "file"} {
		s := app.RuntimeSettings{Project: "lab", Sources: map[string]string{"project": source}}
		selected := runtimeSelection(a, CommandSpec{Path: []string{"article", "show"}}, []string{"bare"}, s, false)
		require.False(t, selected.OriginalProjectPresent)
		require.True(t, selected.EffectiveProjectPresent)
		require.Equal(t, source, selected.ProjectOrigin)
		require.Equal(t, "lab", selected.Project)
	}
	s := app.RuntimeSettings{Project: "", Sources: map[string]string{"project": "cli"}}
	selected := runtimeSelection(a, CommandSpec{Path: []string{"article", "show"}}, []string{"bare"}, s, true)
	require.True(t, selected.OriginalProjectPresent)
	require.False(t, selected.EffectiveProjectPresent)
	command := parserCommand(t, []string{"article", "list"})
	command.SetContext(withRuntime(context.Background(), app.RuntimeSettings{Quiet: true, NoColor: true, Output: "json"}))
	require.True(t, boolFlag(command, "quiet"))
	require.True(t, boolFlag(command, "no-color"))
	require.Equal(t, "json", stringFlag(command, "output"))
	other := parserCommand(t, []string{"article", "list"})
	require.False(t, boolFlag(other, "quiet"))
}

func TestRuntimeProjectAutoPathRespectsEffectiveValue(t *testing.T) {
	root := t.TempDir()
	project := filepath.Join(root, "projects/lab")
	require.NoError(t, os.MkdirAll(filepath.Join(project, "docs/post"), 0755))
	require.NoError(t, os.WriteFile(filepath.Join(project, "mind.yaml"), []byte("schema: '1'\n"), 0644))
	application := &app.Context{RepoRoot: root, CWD: root}
	item := CommandSpec{Path: []string{"article", "show"}}
	empty := app.RuntimeSettings{Sources: map[string]string{"project": "cli"}}
	selected := runtimeSelection(application, item, []string{"projects/lab/docs/post"}, empty, true)
	require.True(t, selected.OriginalProjectPresent)
	require.True(t, selected.EffectiveProjectPresent)
	require.Equal(t, "auto_path", selected.ProjectOrigin)
	require.Equal(t, "docs/post", selected.Args[0])
	for _, source := range []string{"env", "file"} {
		selected := runtimeSelection(application, item, []string{"projects/lab/docs/post"}, app.RuntimeSettings{Project: "other", Sources: map[string]string{"project": source}}, false)
		require.Equal(t, "other", selected.Project)
		require.Equal(t, "projects/lab/docs/post", selected.Args[0])
	}
}
