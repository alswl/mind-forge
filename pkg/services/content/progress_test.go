package content

import (
	"context"
	"testing"

	"github.com/alswl/mind-forge/pkg/common/task"
	"github.com/stretchr/testify/require"
)

func TestLocalGitStageUsesFakeRunnerAndPreservesArguments(t *testing.T) {
	var stages []task.Stage
	ctx := task.WithObserver(context.Background(), func(stage task.Stage) { stages = append(stages, stage) })
	calls := 0
	ops := ProjectOperations{Git: func(_ context.Context, root string, args ...string) ([]byte, []byte, error) {
		calls++
		require.Equal(t, "synthetic-root", root)
		require.Equal(t, []string{"ls-files", "synthetic.md"}, args)
		require.Equal(t, []task.Stage{{Operation: "project", Name: "running local Git"}}, stages)
		return []byte("synthetic.md"), nil, nil
	}}
	out, errout, err := ops.git(ctx, "synthetic-root", "ls-files", "synthetic.md")
	require.NoError(t, err)
	require.Equal(t, "synthetic.md", string(out))
	require.Empty(t, errout)
	require.Equal(t, 1, calls)
}

func TestLifecycleInventoryHasInvocationStage(t *testing.T) {
	root, project := lifecycleFixture(t, "category")
	var stages []task.Stage
	ctx := task.WithObserver(context.Background(), func(stage task.Stage) { stages = append(stages, stage) })
	plan, err := PlanLifecycle(root, project, LifecycleOperation{Context: ctx, Kind: "convert", Layout: "bundle"})
	require.NoError(t, err)
	require.NotEmpty(t, plan.Digest)
	require.Equal(t, []task.Stage{{Operation: "lifecycle", Name: "inventorying content"}}, stages)
}
