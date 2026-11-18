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
