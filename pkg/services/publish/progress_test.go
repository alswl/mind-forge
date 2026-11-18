package publish

import (
	"context"
	"path/filepath"
	"testing"

	"github.com/alswl/mind-forge/pkg/common/task"
	"github.com/stretchr/testify/require"
)

func TestPublishStagesDoNotAutoBuildOrChangeDryRunResult(t *testing.T) {
	root := t.TempDir()
	project := filepath.Join(root, "projects/lab")
	putPublish(t, root, "minds.yaml", "schema: '1'\n")
	putPublish(t, project, "mind.yaml", "schema: '1'\npublish:\n  targets:\n    - name: local\n      type: local\n      path: ../../published\n")
	putPublish(t, project, "mind-index.yaml", "schema: '1'\narticles:\n- title: Synthetic\n  article_path: docs/note.md\n")
	putPublish(t, project, "docs/note.md", "# Synthetic source\n")
	putPublish(t, project, "outputs/note.md", "# Built source\n")
	var stages []task.Stage
	ctx := task.WithObserver(context.Background(), func(stage task.Stage) { stages = append(stages, stage) })
	local := "local"
	result, _, _, err := Run(root, project, "docs/note.md", RunOptions{Context: ctx, Target: &local, DryRun: true})
	require.NoError(t, err)
	require.NotNil(t, result.Local)
	require.True(t, result.Local.DryRun)
	require.Equal(t, []task.Stage{{Operation: "publish", Name: "preparing publication"}, {Operation: "publish", Name: "preparing local output"}}, stages)
}
