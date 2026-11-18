package build

import (
	"bytes"
	"context"
	"errors"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"github.com/alswl/mind-forge/pkg/common/model"
	"github.com/stretchr/testify/require"
)

func TestProcessIORunnerConstructsCommandWithoutLaunching(t *testing.T) {
	input := strings.NewReader("synthetic input")
	var stdout, stderr bytes.Buffer
	sentinel := errors.New("synthetic runner failure")
	calls := 0
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	runner := NewPipelineRunner(ctx, ProcessIO{Stdin: input, Stdout: &stdout, Stderr: &stderr, Run: func(cmd *exec.Cmd) error {
		calls++
		require.Equal(t, []string{"sh", "-c", "synthetic stage"}, cmd.Args)
		require.Same(t, input, cmd.Stdin)
		require.Same(t, &stdout, cmd.Stdout)
		require.Same(t, &stderr, cmd.Stderr)
		require.True(t, cmd.SysProcAttr.Setpgid)
		require.NotNil(t, cmd.Cancel)
		require.Positive(t, cmd.WaitDelay)
		require.Nil(t, cmd.Process)
		raw, err := io.ReadAll(cmd.Stdin)
		require.NoError(t, err)
		require.Equal(t, "synthetic input", string(raw))
		_, err = io.WriteString(cmd.Stdout, "streamed output")
		require.NoError(t, err)
		_, err = io.WriteString(cmd.Stderr, "streamed error")
		require.NoError(t, err)
		return sentinel
	}})
	require.ErrorIs(t, runner("synthetic stage", "ignored-input", "ignored-output"), sentinel)
	require.Equal(t, 1, calls)
	require.Equal(t, "streamed output", stdout.String())
	require.Equal(t, "streamed error", stderr.String())
	// A different invocation cannot inherit these streams or the fake runner.
	otherCalls := 0
	other := NewPipelineRunner(ctx, ProcessIO{Run: func(cmd *exec.Cmd) error {
		otherCalls++
		require.Nil(t, cmd.Stdin)
		require.Nil(t, cmd.Stdout)
		require.Nil(t, cmd.Stderr)
		require.Nil(t, cmd.Process)
		return nil
	}})
	require.NoError(t, other("other stage", "", ""))
	require.Equal(t, 1, otherCalls)
	require.Equal(t, 1, calls)
}

func TestPreparedBuildPassesInvocationProcessIO(t *testing.T) {
	root := buildTestRoot(t)
	putBuildFile(t, root, "mind.yaml", "schema: '1'\n")
	putBuildFile(t, root, "mind-index.yaml", "schema: '1'\narticles:\n- title: Post\n  article_path: docs/post.md\n")
	putBuildFile(t, root, "docs/post.md", "# Post\n")
	putBuildFile(t, root, "assets/flow.svg", "synthetic asset")
	var stdout bytes.Buffer
	calls := 0
	prepared, err := Prepare(root, root, "post", Options{ProcessIO: ProcessIO{Stdout: &stdout, Run: func(cmd *exec.Cmd) error {
		calls++
		require.Same(t, &stdout, cmd.Stdout)
		require.Nil(t, cmd.Process)
		_, err := io.WriteString(cmd.Stdout, "stage output")
		return err
	}}})
	require.NoError(t, err)
	prepared.config.Pipeline = []model.BuildPipelineRule{pipelineRule("svg-png", "svg", "png")}
	_, _, err = prepared.Execute(root, nil)
	require.NoError(t, err)
	require.Equal(t, 1, calls)
	require.Equal(t, "stage output", stdout.String())
	_, err = os.Stat(filepath.Join(root, "assets/flow.png"))
	require.True(t, os.IsNotExist(err), "fake command must not launch a tool or create its output")
}

func TestPipelineDiagnosticsRouteBothStreamsAndQuietRetainsErrors(t *testing.T) {
	for _, quiet := range []bool{false, true} {
		t.Run(map[bool]string{false: "ordinary", true: "quiet"}[quiet], func(t *testing.T) {
			var oldStdout, oldStderr, diagnostics bytes.Buffer
			reader := strings.NewReader("synthetic stdin")
			calls := 0
			original := ProcessIO{Stdin: reader, Stdout: &oldStdout, Stderr: &oldStderr, Run: func(cmd *exec.Cmd) error {
				calls++
				require.Nil(t, cmd.Process)
				require.Same(t, reader, cmd.Stdin)
				require.Same(t, &diagnostics, cmd.Stderr)
				if quiet {
					require.Equal(t, io.Discard, cmd.Stdout)
				} else {
					require.Same(t, &diagnostics, cmd.Stdout)
				}
				_, err := cmd.Stdout.Write([]byte{'o', 0xff})
				require.NoError(t, err)
				_, err = cmd.Stderr.Write([]byte{'e', 0xfe})
				return err
			}}
			mapped := original.Diagnostics(&diagnostics, quiet)
			require.NoError(t, NewPipelineRunner(context.Background(), mapped)("synthetic", "", ""))
			require.Equal(t, 1, calls)
			require.Empty(t, oldStdout.Bytes())
			require.Empty(t, oldStderr.Bytes())
			want := []byte{'o', 0xff, 'e', 0xfe}
			if quiet {
				want = []byte{'e', 0xfe}
			}
			require.Equal(t, want, diagnostics.Bytes())
			require.Same(t, &oldStdout, original.Stdout)
			require.Same(t, &oldStderr, original.Stderr)
		})
	}
}

func TestNilRunnerCompatibilityStreamsAndExplicitQuiet(t *testing.T) {
	streams := defaultPipelineIO()
	streams.Run = func(cmd *exec.Cmd) error {
		require.NotNil(t, cmd.Stdin)
		require.NotNil(t, cmd.Stdout)
		require.NotNil(t, cmd.Stderr)
		require.Same(t, os.Stdin, cmd.Stdin)
		require.Same(t, os.Stdout, cmd.Stdout)
		require.Same(t, os.Stderr, cmd.Stderr)
		require.Nil(t, cmd.Process)
		return nil
	}
	require.NoError(t, NewPipelineRunner(context.Background(), streams)("synthetic-not-launched", "", ""))
	quiet := defaultPipelineIO().Diagnostics(os.Stderr, true)
	quiet.Run = func(cmd *exec.Cmd) error {
		require.NotNil(t, cmd.Stdin)
		require.NotNil(t, cmd.Stdout)
		require.NotNil(t, cmd.Stderr)
		require.Same(t, os.Stdin, cmd.Stdin)
		require.Equal(t, io.Discard, cmd.Stdout)
		require.Same(t, os.Stderr, cmd.Stderr)
		require.Nil(t, cmd.Process)
		return nil
	}
	require.NoError(t, NewPipelineRunner(context.Background(), quiet)("synthetic-not-launched", "", ""))
}
