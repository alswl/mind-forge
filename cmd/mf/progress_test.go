package main

import (
	"bytes"
	"context"
	"testing"

	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/pkg/common/task"
	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/stretchr/testify/require"
)

func TestInvocationObserverDoesNotTrustForcedTTYEnvironment(t *testing.T) {
	var stdout, stderr bytes.Buffer
	observer := invocationObserver(&stdout, &stderr, &app.Context{Format: "text"}, map[string]string{"TERM": "xterm", "MF_FORCE_TTY": "1", "NO_COLOR": "1"})
	require.Nil(t, observer)
	require.Empty(t, stderr.String())
}

func TestFakeTTYObserverWritesOnlyPlainDiagnostics(t *testing.T) {
	var stderr bytes.Buffer
	observer := progressObserver(output.ProgressInput{StdoutIsTTY: true, StderrIsTTY: true, Format: "text", Term: "xterm"}, &stderr)
	require.NotNil(t, observer)
	task.Notify(task.WithObserver(context.Background(), observer), task.Stage{Operation: "build", Name: "preparing inputs"})
	require.Equal(t, "build: preparing inputs\n", stderr.String())
	require.NotContains(t, stderr.String(), "\x1b")
	observer = progressObserver(output.ProgressInput{StdoutIsTTY: true, StderrIsTTY: true, Format: "json", Term: "xterm"}, &stderr)
	task.Notify(task.WithObserver(context.Background(), observer), task.Stage{Operation: "build", Name: "hidden"})
	require.Equal(t, "build: preparing inputs\n", stderr.String())
}
