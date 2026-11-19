package lifecycle

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	"github.com/alswl/mind-forge/pkg/common/task"
	manager "github.com/alswl/mind-forge/pkg/managers/lifecycle"
	"github.com/stretchr/testify/require"
)

func TestLifecycleStagesPreserveDurableTransactionResult(t *testing.T) {
	root := t.TempDir()
	require.NoError(t, os.WriteFile(filepath.Join(root, "note.md"), []byte("before"), 0644))
	before := manager.Snapshot{"note.md": {Kind: "file", Bytes: []byte("before"), Mode: 0644}}
	after := manager.Snapshot{"note.md": {Kind: "file", Bytes: []byte("after"), Mode: 0644}}
	plan, err := manager.NewPlan(root, "synthetic", []string{"."}, before, after)
	require.NoError(t, err)
	var stages []task.Stage
	ctx := task.WithObserver(context.Background(), func(stage task.Stage) { stages = append(stages, stage) })
	result, err := New().Execute(ctx, root, plan, false, false, true)
	require.NoError(t, err)
	require.Equal(t, "complete", result["phase"])
	raw, err := os.ReadFile(filepath.Join(root, "note.md"))
	require.NoError(t, err)
	require.Equal(t, "after", string(raw))
	require.Equal(t, []task.Stage{{Operation: "lifecycle", Name: "validating plan"}, {Operation: "lifecycle", Name: "preparing transaction"}, {Operation: "lifecycle", Name: "saving transaction backup"}, {Operation: "lifecycle", Name: "applying changes"}}, stages)
}
