package content

import (
	"context"
	"testing"

	"github.com/alswl/mind-forge/pkg/common/outcome"
	"github.com/stretchr/testify/require"
)

func TestFinalLintLiteralRulePreservesSourceLines(t *testing.T) {
	project := canonicalPath(t.TempDir())
	putContent(t, project, "mind.yaml", `schema: '1'
article_lint:
  final:
    rules:
      - literal: 'marker.*'
        suggestion: Replace this marker.
`)
	putContent(t, project, "docs/note.md", "---\r\ntitle: marker.*\r\n---\r\n"+
		"# Example\r\nmarker-extra\r\n> [!mf-private]\r\n> marker.*\r\n"+
		"\r\n> marker.*\r\n\r\nmarker.*")
	issues, err := LintFinalArticles(context.Background(), project, nil)
	require.NoError(t, err)
	require.Len(t, issues, 1)
	require.Equal(t, "docs/note.md", issues[0].Path)
	require.Equal(t, 11, issues[0].Line)
	require.Equal(t, "final_leak", issues[0].Kind)
	require.Contains(t, issues[0].Message, "Replace this marker.")
}

func TestFinalLintUsesRegisteredStatusForConfiguredDirectory(t *testing.T) {
	project := canonicalPath(t.TempDir())
	putContent(t, project, "mind.yaml", "schema: '1'\nbuild:\n  articles:\n    post: {article_dir: authored}\n")
	putContent(t, project, "mind-index.yaml", "schema: '1'\narticles:\n- title: Post\n  article_path: authored/\n  status: completed\n")
	putContent(t, project, "authored/post.md", "# Post\nW37\n")
	putContent(t, project, "authored/.extra.md", "W38\n")
	draft := "draft"
	issues, err := LintFinalArticles(context.Background(), project, &draft)
	require.NoError(t, err)
	require.Empty(t, issues)
	completed := "completed"
	issues, err = LintFinalArticles(context.Background(), project, &completed)
	require.NoError(t, err)
	require.Len(t, issues, 2)
	require.Equal(t, "authored/.extra.md", issues[0].Path)
	require.Equal(t, "authored/post.md", issues[1].Path)
}

func TestMalformedFinalRulesAreIgnoredByStandardLint(t *testing.T) {
	project := canonicalPath(t.TempDir())
	putContent(t, project, "docs/post.md", "# Post\n")
	for _, rule := range []string{"null", "{literal: 123, suggestion: Edit}", "{literal: W37, suggestion: false}"} {
		putContent(t, project, "mind.yaml", "schema: '1'\narticle_lint:\n  final:\n    rules: ["+rule+"]\n")
		_, err := LintArticles(project, false)
		require.NoError(t, err)
		_, err = LintFinalArticles(context.Background(), project, nil)
		var failure *outcome.Error
		require.ErrorAs(t, err, &failure)
		require.Equal(t, 2, failure.ExitCode())
	}
}
