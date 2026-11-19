package build

import (
	"context"
	"io"
	"os/exec"

	"github.com/alswl/mind-forge/pkg/common/task"
)

// ProcessIO belongs to one invocation. Run can inspect a constructed command
// without launching it, while production uses the process-group-aware runner.
type ProcessIO struct {
	// Explicit distinguishes an intentionally all-nil IO (discard) from Prepare defaults.
	// Any non-nil stream or Run also makes the complete IO group explicit.
	Explicit bool
	Stdin    io.Reader
	Stdout   io.Writer
	Stderr   io.Writer
	Run      func(*exec.Cmd) error
}

func NewPipelineRunner(ctx context.Context, streams ProcessIO) PipelineRunner {
	return func(command, _, _ string) error {
		cmd := task.Command(ctx, "sh", "-c", command)
		cmd.Stdin = streams.Stdin
		cmd.Stdout = streams.Stdout
		cmd.Stderr = streams.Stderr
		if streams.Run != nil {
			return streams.Run(cmd)
		}
		return cmd.Run()
	}
}

// Diagnostics keeps child logs separate from the command's result stream.
// Quiet suppresses ordinary child output while preserving its error stream.
func (streams ProcessIO) Diagnostics(stderr io.Writer, quiet bool) ProcessIO {
	streams.Explicit = true
	streams.Stdout = stderr
	streams.Stderr = stderr
	if quiet {
		streams.Stdout = io.Discard
	}
	return streams
}

func defaultPipelineIO() ProcessIO {
	stdin, stdout, stderr := task.ProcessStreams()
	return ProcessIO{Stdin: stdin, Stdout: stdout, Stderr: stderr}
}

func (streams ProcessIO) preparedStreams() ProcessIO {
	if streams.Explicit || streams.Stdin != nil || streams.Stdout != nil || streams.Stderr != nil || streams.Run != nil {
		return streams
	}
	return defaultPipelineIO()
}
