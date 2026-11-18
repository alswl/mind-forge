package main

import (
	"bytes"
	"context"
	"io"
	"os/exec"
	"testing"

	app "github.com/alswl/mind-forge/pkg/config"
	"github.com/alswl/mind-forge/pkg/services/build"
	"github.com/stretchr/testify/require"
)

func TestBuildOptionsInjectsInvocationDiagnosticsPolicy(t *testing.T) {
	for _, quiet := range []bool{false, true} {
		command := parserCommand(t, []string{"build"})
		require.NoError(t, command.ParseFlags([]string{"--dry-run"}))
		command.SetContext(context.Background())
		var original, diagnostics bytes.Buffer
		calls := 0
		streams := build.ProcessIO{Stdout: &original, Stderr: &original, Run: func(cmd *exec.Cmd) error {
			calls++
			require.Nil(t, cmd.Process)
			_, err := io.WriteString(cmd.Stdout, "ordinary")
			require.NoError(t, err)
			_, err = io.WriteString(cmd.Stderr, "diagnostic")
			return err
		}}
		options := buildOptions(command, &app.Context{Quiet: quiet}, streams, &diagnostics)
		require.True(t, options.DryRun)
		require.Equal(t, command.Context(), options.Context)
		require.NoError(t, build.NewPipelineRunner(options.Context, options.ProcessIO)("synthetic", "", ""))
		want := "ordinarydiagnostic"
		if quiet {
			want = "diagnostic"
		}
		require.Equal(t, want, diagnostics.String())
		require.Empty(t, original.String())
		require.Equal(t, 1, calls)
	}
}
