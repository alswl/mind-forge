package main

import (
	"bytes"
	"strings"
	"testing"

	"github.com/stretchr/testify/require"
)

func TestEveryPublicCommandHasUsefulExamples(t *testing.T) {
	spec, err := LoadSpec()
	require.NoError(t, err)
	for _, item := range spec.Commands {
		t.Run(strings.Join(item.Path, " "), func(t *testing.T) {
			require.NotEmpty(t, item.Examples, "public command lacks example")
			for _, example := range item.Examples {
				require.NotEmpty(t, example.Prerequisites)
				require.NotEmpty(t, example.Effect)
				require.True(t, strings.HasPrefix(example.Command, "mf "))
				require.NotContains(t, example.Command, "--help", "example must demonstrate command behavior")
				prefix := strings.TrimSpace("mf " + strings.Join(item.Path, " "))
				require.True(t, strings.HasPrefix(example.Command, prefix), "example belongs to another command")
			}
		})
	}
}

func TestHelpAndManualShareStructuredExamples(t *testing.T) {
	spec, err := LoadSpec()
	require.NoError(t, err)
	root, err := NewCommand(nil)
	require.NoError(t, err)
	manual := Manual(spec)
	for _, item := range spec.Commands {
		c, remaining, err := root.Find(item.Path)
		require.NoError(t, err)
		require.Empty(t, remaining)
		var help bytes.Buffer
		c.SetOut(&help)
		require.NoError(t, c.Help())
		shared := ExamplesText(item.Examples)
		require.NotEmpty(t, shared)
		require.Contains(t, help.String(), shared)
		require.Contains(t, manual, shared)
	}
}

func TestLifecyclePlanExampleKeepsExportsInsideFreshWorkspace(t *testing.T) {
	spec, err := LoadSpec()
	require.NoError(t, err)
	found := false
	for _, item := range spec.Commands {
		if strings.Join(item.Path, " ") != "lifecycle apply" {
			continue
		}
		found = true
		for _, example := range item.Examples {
			require.GreaterOrEqual(t, len(example.Prerequisites), 3)
			require.Equal(t, "mf_example_workspace=$(mktemp -d)", example.Prerequisites[0])
			require.Equal(t, `mkdir "$mf_example_workspace/repo" "$mf_example_workspace/exports"`, example.Prerequisites[1])
			require.Equal(t, `cd "$mf_example_workspace/repo"`, example.Prerequisites[2])
			all := strings.Join(example.Prerequisites, "\n") + "\n" + example.Command
			require.NotContains(t, all, "../preview.json")
			require.NotContains(t, all, "../plan.json")
			require.Contains(t, all, "../exports/preview.json")
			require.Contains(t, all, "../exports/plan.json")
		}
	}
	require.True(t, found)
}
