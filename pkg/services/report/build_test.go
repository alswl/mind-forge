package report

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"sync"
	"testing"

	"github.com/stretchr/testify/require"
)

type workerFunc func(context.Context, Job) (Receipt, error)

func (run workerFunc) Run(ctx context.Context, job Job) (Receipt, error) { return run(ctx, job) }

func receiptFor(job Job) Receipt {
	return Receipt{Protocol: job.Protocol, Stage: job.Stage, Result: Result{
		Protocol: job.Protocol, Artifact: job.Artifact, View: job.View,
		Renderer: job.Renderer.ID, RendererVersion: job.Renderer.Version,
	}}
}

func writeFile(t *testing.T, root, path, raw string) {
	t.Helper()
	path = filepath.Join(root, path)
	require.NoError(t, os.MkdirAll(filepath.Dir(path), 0755))
	require.NoError(t, os.WriteFile(path, []byte(raw), 0644))
}

func standalone(t *testing.T) (Scope, Request) {
	t.Helper()
	scope, err := StandaloneScope(t.TempDir())
	require.NoError(t, err)
	writeFile(t, scope.Directory, "report.tsx", "// worker input")
	writeFile(t, scope.Directory, "data.json", `{"title":"Synthetic"}`)
	return scope, Request{Entry: "report.tsx", Data: "data.json"}
}

func TestPlanDoesNotStartWorkerOrReadData(t *testing.T) {
	scope, request := standalone(t)
	writeFile(t, scope.Directory, "data.json", "malformed")
	service := Service{Worker: workerFunc(func(context.Context, Job) (Receipt, error) {
		t.Error("unexpected worker invocation")
		return Receipt{}, nil
	})}
	plan, err := service.Plan(t.Context(), scope, request)
	require.NoError(t, err)
	require.Equal(t, "outputs/report.chart.html", plan.Artifact)
	require.NoDirExists(t, filepath.Join(scope.Directory, "outputs"))
	_, err = service.Build(t.Context(), scope, request)
	require.ErrorIs(t, err, ErrInvalid)
	require.ErrorContains(t, err, "data.json")
	writeFile(t, scope.Directory, "data.json", `{"title":12}`)
	_, err = service.Build(t.Context(), scope, request)
	require.ErrorIs(t, err, ErrInvalid)
	require.ErrorContains(t, err, "/title")
}

func TestBuiltinRendererDataContractsInBothViews(t *testing.T) {
	for _, renderer := range []string{"builtin/trend", "builtin/comparison", "builtin/brief"} {
		for _, view := range []string{"chart", "table"} {
			t.Run(renderer+"/"+view, func(t *testing.T) {
				scope, request := standalone(t)
				request.Renderer, request.View = renderer, view
				artifact := "outputs/report." + view + ".html"
				writeFile(t, scope.Directory, artifact, "previous")
				calls := 0
				service := Service{Worker: workerFunc(func(_ context.Context, job Job) (Receipt, error) {
					calls++
					require.NoError(t, os.WriteFile(job.Stage, []byte("rendered"), 0600))
					return receiptFor(job), nil
				})}
				_, err := service.Build(t.Context(), scope, request)
				if renderer == "builtin/brief" {
					require.NoError(t, err)
					require.Equal(t, 1, calls)
					writeFile(t, scope.Directory, "data.json", `{"title":"Synthetic","rows":[]}`)
					_, err = service.Build(t.Context(), scope, request)
					require.ErrorIs(t, err, ErrInvalid)
					require.Equal(t, 1, calls)
					return
				}
				require.ErrorIs(t, err, ErrInvalid)
				for _, field := range []string{"rows", "x", "series"} {
					require.ErrorContains(t, err, "'"+field+"'")
				}
				require.Zero(t, calls)
				raw, err := os.ReadFile(filepath.Join(scope.Directory, artifact))
				require.NoError(t, err)
				require.Equal(t, "previous", string(raw))

				catalog, err := service.Catalog(Renderer, renderer)
				require.NoError(t, err)
				writeFile(t, scope.Directory, "data.json", string(catalog.Items[0].ExampleData))
				_, err = service.Build(t.Context(), scope, request)
				require.NoError(t, err)
				require.Equal(t, 1, calls)
			})
		}
	}
}

func TestBuildKeepsPriorArtifactAndCleansStagingOnFailure(t *testing.T) {
	const path = "outputs/report.chart.html"
	for _, mode := range []string{"error", "cancel", "protocol", "artifact", "stage", "replace", "symlink", "empty"} {
		t.Run(mode, func(t *testing.T) {
			scope, request := standalone(t)
			writeFile(t, scope.Directory, path, "previous")
			ctx, cancel := context.WithCancel(t.Context())
			defer cancel()
			service := Service{Worker: workerFunc(func(_ context.Context, job Job) (Receipt, error) {
				require.Equal(t, filepath.Join(scope.Directory, "outputs"), filepath.Dir(job.Stage))
				receipt := receiptFor(job)
				if mode != "empty" {
					require.NoError(t, os.WriteFile(job.Stage, []byte("candidate"), 0600))
				}
				switch mode {
				case "error":
					return Receipt{}, fmt.Errorf("controlled failure")
				case "cancel":
					cancel()
				case "protocol":
					receipt.Protocol = "wrong"
				case "artifact":
					receipt.Result.Artifact = "outputs/other.html"
				case "stage":
					receipt.Stage = filepath.Join(scope.Directory, path)
				case "replace":
					require.NoError(t, os.Remove(job.Stage))
					require.NoError(t, os.WriteFile(job.Stage, []byte("replacement"), 0600))
				case "symlink":
					require.NoError(t, os.Remove(job.Stage))
					require.NoError(t, os.Symlink(filepath.Join(scope.Directory, path), job.Stage))
				}
				return receipt, nil
			})}
			_, err := service.Build(ctx, scope, request)
			require.Error(t, err)
			if mode == "cancel" {
				require.ErrorIs(t, err, context.Canceled)
			}
			requireArtifact(t, scope, "previous")
		})
	}
}

func TestConcurrentBuildsUseDistinctStagesAndLastSuccessWins(t *testing.T) {
	scope, request := standalone(t)
	entered := make(chan Job, 2)
	first, second := make(chan struct{}), make(chan struct{})
	service := Service{Worker: workerFunc(func(ctx context.Context, job Job) (Receipt, error) {
		if err := os.WriteFile(job.Stage, []byte(job.Theme), 0600); err != nil {
			return Receipt{}, err
		}
		entered <- job
		release := first
		if job.Theme == "dark" {
			release = second
		}
		select {
		case <-release:
			return receiptFor(job), nil
		case <-ctx.Done():
			return Receipt{}, ctx.Err()
		}
	})}
	var builds sync.WaitGroup
	ctx, cancel := context.WithCancel(t.Context())
	defer func() { cancel(); builds.Wait() }()
	start := func(theme string) <-chan error {
		done := make(chan error, 1)
		builds.Go(func() {
			req := request
			req.Theme = theme
			_, err := service.Build(ctx, scope, req)
			done <- err
		})
		return done
	}
	firstDone, secondDone := start("light"), start("dark")
	stage := func() Job {
		t.Helper()
		select {
		case job := <-entered:
			return job
		case err := <-firstDone:
			t.Fatalf("first build failed before worker: %v", err)
		case err := <-secondDone:
			t.Fatalf("second build failed before worker: %v", err)
		}
		return Job{}
	}
	one, two := stage(), stage()
	require.NotEqual(t, one.Stage, two.Stage)
	close(second)
	require.NoError(t, <-secondDone)
	raw, err := os.ReadFile(filepath.Join(scope.Directory, "outputs/report.chart.html"))
	require.NoError(t, err)
	require.Equal(t, "dark", string(raw))
	close(first)
	require.NoError(t, <-firstDone)
	requireArtifact(t, scope, "light")
}

func requireArtifact(t *testing.T, scope Scope, want string) {
	t.Helper()
	output := filepath.Join(scope.Directory, "outputs")
	raw, err := os.ReadFile(filepath.Join(output, "report.chart.html"))
	require.NoError(t, err)
	require.Equal(t, want, string(raw))
	entries, err := os.ReadDir(output)
	require.NoError(t, err)
	require.Len(t, entries, 1)
}
