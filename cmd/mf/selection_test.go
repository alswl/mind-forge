package main

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/stretchr/testify/require"
)

func TestSelectionRetainsOriginalAndEffectivePresence(t *testing.T) {
	root, err := filepath.EvalSymlinks(t.TempDir())
	require.NoError(t, err)
	project := filepath.Join(root, "projects/lab")
	require.NoError(t, os.MkdirAll(filepath.Join(project, "docs/post"), 0755))
	require.NoError(t, os.WriteFile(filepath.Join(project, "mind.yaml"), []byte("schema: '1'\n"), 0644))
	application := &app.Context{RepoRoot: root, CWD: root}
	item := CommandSpec{Path: []string{"article", "show"}}
	args := []string{"projects/lab/docs/post"}
	selection := resolveSelection(application, item, args, "", false)
	require.False(t, selection.OriginalProjectPresent)
	require.True(t, selection.EffectiveProjectPresent)
	require.Equal(t, "auto_path", selection.ProjectOrigin)
	require.Equal(t, project, selection.Project)
	require.Equal(t, []string{"docs/post"}, selection.Args)
	require.Equal(t, []string{"projects/lab/docs/post"}, args)
	explicit := resolveSelection(application, item, args, "other", true)
	require.True(t, explicit.OriginalProjectPresent)
	require.True(t, explicit.EffectiveProjectPresent)
	require.Equal(t, "explicit", explicit.ProjectOrigin)
	require.Equal(t, "other", explicit.Project)
	require.Equal(t, args, explicit.Args)
	absent := resolveSelection(application, item, []string{"bare"}, "", false)
	require.False(t, absent.EffectiveProjectPresent)
	require.Equal(t, "none", absent.ProjectOrigin)
}

func TestSelectionAdapterDoesNotMutateFlagsOrOtherTrees(t *testing.T) {
	first, err := NewCommand(nil)
	require.NoError(t, err)
	second, err := NewCommand(nil)
	require.NoError(t, err)
	one, _, err := first.Find([]string{"article", "show"})
	require.NoError(t, err)
	two, _, err := second.Find([]string{"article", "show"})
	require.NoError(t, err)
	require.False(t, flagPresent(one, "project"))
	selection := ResolvedSelection{EffectiveProjectPresent: true, ProjectOrigin: "auto_path", Project: "projects/lab", Args: []string{"docs/post"}}
	one.SetContext(withSelection(context.Background(), selection))
	require.True(t, flagPresent(one, "project"))
	require.Equal(t, "projects/lab", stringFlag(one, "project"))
	require.NotNil(t, optionalFlag(one, "project"))
	require.Equal(t, "projects/lab", *optionalFlag(one, "project"))
	require.False(t, one.Flags().Lookup("project").Changed)
	raw, err := one.Flags().GetString("project")
	require.NoError(t, err)
	require.Empty(t, raw)
	require.False(t, flagPresent(two, "project"))
	require.Nil(t, optionalFlag(two, "project"))
	stored, ok := selectionFromCommand(one)
	require.True(t, ok)
	require.False(t, stored.OriginalProjectPresent)
	one.SetContext(context.Background())
	require.False(t, flagPresent(one, "project"))
	require.Nil(t, optionalFlag(one, "project"))
}
