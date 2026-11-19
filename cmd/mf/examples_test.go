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
				command := strings.TrimSpace(example.Command)
				if strings.HasPrefix(command, "(\n") && strings.HasSuffix(command, "\n)") {
					lines := strings.Split(command, "\n")
					command = strings.TrimSpace(lines[len(lines)-2])
				}
				require.True(t, strings.HasPrefix(command, "mf "))
				require.NotContains(t, example.Command, "--help", "example must demonstrate command behavior")
				prefix := strings.TrimSpace("mf " + strings.Join(item.Path, " "))
				require.True(t, strings.HasPrefix(command, prefix), "example belongs to another command")
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
			// This checks the advertised copyable shape, not real shell execution.
			for _, prerequisite := range example.Prerequisites {
				require.True(t, strings.HasPrefix(prerequisite, "#"), "setup must live in the complete guarded Command")
			}
			steps := strings.Split(example.Command, "\n")
			require.GreaterOrEqual(t, len(steps), 5)
			require.Equal(t, "(", steps[0])
			require.Equal(t, ")", steps[len(steps)-1])
			require.Equal(t, "mf_example_workspace=$(mktemp -d) &&", strings.TrimSpace(steps[1]))
			require.Equal(t, `mkdir "$mf_example_workspace/repo" "$mf_example_workspace/exports" &&`, strings.TrimSpace(steps[2]))
			require.Equal(t, `cd "$mf_example_workspace/repo" &&`, strings.TrimSpace(steps[3]))
			for _, step := range steps[1 : len(steps)-2] {
				require.True(t, strings.HasSuffix(strings.TrimSpace(step), " &&"), "no fall-through after setup or prerequisite failure")
			}
			require.True(t, strings.HasPrefix(strings.TrimSpace(steps[len(steps)-2]), "mf lifecycle apply "))
			require.NotContains(t, example.Command, "../preview.json")
			require.NotContains(t, example.Command, "../plan.json")
			require.Contains(t, example.Command, "../exports/preview.json")
			require.Contains(t, example.Command, "../exports/plan.json")
			require.NotContains(t, example.Command, "exit ")
		}
	}
	require.True(t, found)
}
