package main

import (
	"io"
	"os"

	"github.com/alswl/mind-forge/internal/output"
	"github.com/alswl/mind-forge/pkg/common/task"
	app "github.com/alswl/mind-forge/pkg/config"
	"golang.org/x/term"
)

func realTerminal(writer io.Writer) bool {
	file, ok := writer.(*os.File)
	return ok && term.IsTerminal(int(file.Fd()))
}

func invocationObserver(stdout, stderr io.Writer, application *app.Context, env map[string]string) task.Observer {
	return progressObserver(output.ProgressInput{StdoutIsTTY: realTerminal(stdout), StderrIsTTY: realTerminal(stderr), Format: application.Format, Quiet: application.Quiet, Term: env["TERM"]}, stderr)
}

func progressObserver(policy output.ProgressInput, stderr io.Writer) task.Observer {
	if !output.ProgressEnabled(policy) {
		return nil
	}
	return func(stage task.Stage) {
		// Status feedback is best effort; a diagnostic writer cannot change the
		// operation's durable result or cancellation/recovery policy.
		_, _ = io.WriteString(stderr, output.ProgressText(stage))
	}
}
