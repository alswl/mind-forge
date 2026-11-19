package term

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	"github.com/alswl/mind-forge/pkg/common/model"
	"github.com/alswl/mind-forge/pkg/common/task"
	"github.com/stretchr/testify/require"
)

func TestCorrectionScanStagesPreserveAuthorBytesOnPreview(t *testing.T) {
	root := t.TempDir()
	target := filepath.Join(root, "note.md")
	require.NoError(t, os.WriteFile(target, []byte("gatway\n"), 0640))
	var stages []task.Stage
	ctx := task.WithObserver(context.Background(), func(stage task.Stage) { stages = append(stages, stage) })
	report, err := LintFiles(root, target, []model.Term{termRule("GATEWAY", "gatway", "gateway", "word", "standalone")}, LintOptions{Context: ctx, Fix: true, DryRun: true})
	require.NoError(t, err)
	require.NotNil(t, report.WouldFixCount)
	require.Equal(t, uint64(1), *report.WouldFixCount)
	raw, err := os.ReadFile(target)
	require.NoError(t, err)
	require.Equal(t, "gatway\n", string(raw))
	require.Equal(t, []task.Stage{{Operation: "term", Name: "discovering scan files"}, {Operation: "term", Name: "scanning corrections"}}, stages)
}
