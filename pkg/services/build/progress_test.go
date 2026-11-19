package build

import (
	"context"
	"os"
	"testing"

	"github.com/alswl/mind-forge/pkg/common/model"
	"github.com/alswl/mind-forge/pkg/common/task"
	"github.com/stretchr/testify/require"
)

func TestBuildStagesPreserveOutputWithFakePipeline(t *testing.T) {
	root := buildTestRoot(t)
	putBuildFile(t, root, "mind.yaml", "schema: '1'\n")
	putBuildFile(t, root, "mind-index.yaml", "schema: '1'\narticles:\n- title: Post\n  article_path: docs/post.md\n")
	putBuildFile(t, root, "docs/post.md", "# Post\n")
	putBuildFile(t, root, "assets/flow.svg", "synthetic")
	var stages []task.Stage
	ctx := task.WithObserver(context.Background(), func(stage task.Stage) { stages = append(stages, stage) })
	prepared, err := Prepare(root, root, "post", Options{Context: ctx})
	require.NoError(t, err)
	prepared.config.Pipeline = []model.BuildPipelineRule{pipelineRule("svg-png", "svg", "png")}
	calls := 0
	_, _, err = prepared.Execute(root, func(_, _, out string) error {
		calls++
		return os.WriteFile(out, []byte("synthetic generated asset"), 0644)
	})
	require.NoError(t, err)
	require.Equal(t, 1, calls)
	raw, err := os.ReadFile(prepared.Plan.OutputPath)
	require.NoError(t, err)
	require.Equal(t, "# Post\n", string(raw))
	require.Equal(t, []task.Stage{{Operation: "build", Name: "preparing inputs"}, {Operation: "build", Name: "assembling content"}, {Operation: "build", Name: "writing output"}, {Operation: "build", Name: "running asset transform"}}, stages)
}
