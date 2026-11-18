package build

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"testing"

	"github.com/alswl/mind-forge/pkg/common/model"
	"github.com/stretchr/testify/require"
)

func TestCancelledBuildDoesNotCreateOutput(t *testing.T) {
	root := buildTestRoot(t)
	putBuildFile(t, root, "mind.yaml", "schema: '1'\n")
	putBuildFile(t, root, "mind-index.yaml", "schema: '1'\narticles:\n- title: Post\n  article_path: docs/post.md\n")
	putBuildFile(t, root, "docs/post.md", "# Post\n")
	ctx, cancel := context.WithCancel(context.Background())
	prepared, err := Prepare(root, root, "post", Options{Context: ctx})
	require.NoError(t, err)
	cancel()
	_, _, err = prepared.Execute(root, nil)
	require.ErrorIs(t, err, context.Canceled)
	_, err = os.Stat(prepared.Plan.OutputPath)
	require.True(t, errors.Is(err, os.ErrNotExist))
}

func TestCancelledPipelineRestoresFailedStageAndStopsChain(t *testing.T) {
	root := t.TempDir()
	assets := filepath.Join(root, "assets")
	require.NoError(t, os.Mkdir(assets, 0755))
	require.NoError(t, os.WriteFile(filepath.Join(assets, "flow.d2"), []byte("source"), 0644))
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	calls := 0
	rules := []model.BuildPipelineRule{pipelineRule("d2-svg", "d2", "svg"), pipelineRule("svg-png", "svg", "png")}
	_, _, err := RunPipelineContext(ctx, root, assets, rules, false, func(_, _, out string) error {
		calls++
		require.NoError(t, os.WriteFile(out, []byte("partial"), 0644))
		cancel()
		return context.Canceled
	})
	require.ErrorIs(t, err, context.Canceled)
	require.Equal(t, 1, calls)
	_, err = os.Stat(filepath.Join(assets, "flow.svg"))
	require.ErrorIs(t, err, os.ErrNotExist)
	_, err = os.Stat(filepath.Join(assets, "flow.png"))
	require.ErrorIs(t, err, os.ErrNotExist)
}
