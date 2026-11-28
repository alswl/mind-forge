package report

import (
	"context"
	"io"
	"os"
	"testing"

	"github.com/stretchr/testify/require"
)

func TestDefaultWorkerWithoutNodePreservesOutput(t *testing.T) {
	t.Setenv("PATH", t.TempDir())
	scope, request := standalone(t)
	writeFile(t, scope.Directory, "outputs/report.chart.html", "previous")
	service := Service{Worker: ProcessWorker{Stderr: io.Discard}}
	_, err := service.Plan(t.Context(), scope, request)
	require.NoError(t, err)
	_, err = service.Build(t.Context(), scope, request)
	require.ErrorIs(t, err, ErrUnavailable)
	require.ErrorContains(t, err, "node executable not found on PATH")
	requireArtifact(t, scope, "previous")
}

func TestCatalogUsesRegistryComponentExamples(t *testing.T) {
	scope, _ := standalone(t)
	service := Service{Worker: workerFunc(func(_ context.Context, job Job) (Receipt, error) {
		require.Equal(t, "outputs/catalog.html", job.Artifact)
		require.NotEmpty(t, job.Catalog)
		require.Equal(t, "builtin/bar", job.Catalog[0].ID)
		require.Equal(t, "embedded:example/bar", job.Catalog[0].Example)
		require.NoError(t, os.WriteFile(job.Stage, []byte("catalog"), 0600))
		return receiptFor(job), nil
	})}
	result, err := service.BuildCatalog(t.Context(), scope, "", "")
	require.NoError(t, err)
	require.Equal(t, "outputs/catalog.html", result.Artifact)
}
