package content

import (
	"context"
	"testing"

	"github.com/stretchr/testify/require"
)

func TestFinalLintCanUseOnlyProjectRulesAndPreservesSourceLines(t *testing.T) {
	project := canonicalPath(t.TempDir())
	putContent(t, project, "mind.yaml", `schema: '1'
article_lint:
  final:
    disable_defaults: true
    rules:
      - literal: 'marker.*'
        suggestion: Replace this marker.
`)
	putContent(t, project, "docs/note.md", "---\r\ntitle: marker.*\r\n---\r\n"+
		"# Example\r\nW37 marker-extra\r\n> [!mf-private]\r\n> marker.*\r\n"+
		"\r\n> marker.*\r\n\r\nmarker.*")
	issues, err := LintFinalArticles(context.Background(), project, nil)
	require.NoError(t, err)
	require.Len(t, issues, 1)
	require.Equal(t, "docs/note.md", issues[0].Path)
	require.Equal(t, 11, issues[0].Line)
	require.Equal(t, "final_leak", issues[0].Kind)
	require.Contains(t, issues[0].Message, "Replace this marker.")
}
