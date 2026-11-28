package report

import (
	"context"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/stretchr/testify/require"
)

func script(t *testing.T, scope Scope, body string) string {
	t.Helper()
	path := filepath.Join(scope.Directory, "worker")
	require.NoError(t, os.WriteFile(path, []byte("#!/bin/sh\n"+body), 0700))
	return path
}

func TestProcessWorkerRequiresOneStrictReceipt(t *testing.T) {
	scope, _ := standalone(t)
	for _, body := range []string{
		"printf '%s' '{\"unexpected\":1}'",
		"printf '%s' '{} {}'",
		"printf '%s' 'invalid'",
		"exit 42",
	} {
		t.Run(body, func(t *testing.T) {
			worker := ProcessWorker{Executable: script(t, scope, body), Stderr: io.Discard}
			_, err := worker.Run(t.Context(), Job{Protocol: Protocol, WorkDir: scope.Directory})
			require.Error(t, err)
		})
	}
	worker := ProcessWorker{Executable: script(t, scope, "cat >/dev/null\nprintf '%s' '{\"protocol\":\"report.mind-forge/v1\"}'"), Stderr: io.Discard}
	receipt, err := worker.Run(t.Context(), Job{Protocol: Protocol, WorkDir: scope.Directory})
	require.NoError(t, err)
	require.Equal(t, Protocol, receipt.Protocol)
}

func TestCancelProcessWorkerKillsDescendantsAndKeepsArtifact(t *testing.T) {
	scope, request := standalone(t)
	writeFile(t, scope.Directory, "outputs/report.chart.html", "previous")
	worker := ProcessWorker{Executable: script(t, scope, `
(sleep 1; printf survived > survived) &
printf ready > ready
wait
`), Stderr: io.Discard}
	ctx, cancel := context.WithCancel(t.Context())
	defer cancel()
	done := make(chan error, 1)
	go func() {
		_, err := (Service{Worker: worker}).Build(ctx, scope, request)
		done <- err
	}()
	require.Eventually(t, func() bool {
		_, err := os.Stat(filepath.Join(scope.Directory, "ready"))
		return err == nil
	}, 3*time.Second, 5*time.Millisecond)
	cancel()
	select {
	case err := <-done:
		require.ErrorIs(t, err, context.Canceled)
	case <-time.After(3 * time.Second):
		t.Fatal("worker process did not exit on cancellation")
	}
	requireArtifact(t, scope, "previous")
	require.Never(t, func() bool {
		_, err := os.Stat(filepath.Join(scope.Directory, "survived"))
		return err == nil
	}, 1100*time.Millisecond, 15*time.Millisecond, fmt.Sprintf("descendant survived in %s", scope.Directory))
}
