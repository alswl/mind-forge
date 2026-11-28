package report

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strings"

	"github.com/alswl/mind-forge/pkg/common/task"
)

type Module struct {
	ID      string          `json:"id"`
	Version string          `json:"version"`
	Entry   string          `json:"entry"`
	Schema  json.RawMessage `json:"schema"`
}

type CatalogComponent struct {
	Module
	Description string `json:"description,omitempty"`
	Example     string `json:"example"`
}

type Job struct {
	Fragment   *Fragment          `json:"fragment,omitempty"`
	Protocol   string             `json:"protocol"`
	WorkDir    string             `json:"workDir"`
	Entry      string             `json:"entry"`
	Data       string             `json:"data"`
	Renderer   Module             `json:"renderer"`
	Components []Module           `json:"components"`
	View       string             `json:"view"`
	Theme      string             `json:"theme"`
	Stage      string             `json:"stage"`
	Artifact   string             `json:"artifact"`
	Catalog    []CatalogComponent `json:"catalog,omitempty"`
}

type Receipt struct {
	Protocol string `json:"protocol"`
	Stage    string `json:"stage"`
	Result   Result `json:"result"`
}

type Worker interface {
	Run(context.Context, Job) (Receipt, error)
}

func (plan BuildPlan) job(stage string) Job {
	module := func(item Item) Module {
		return Module{ID: item.ID, Version: item.Version, Entry: item.Entry, Schema: item.Schema}
	}
	components := make([]Module, 0, len(plan.Components))
	for _, component := range plan.Components {
		components = append(components, module(component))
	}
	return Job{
		Protocol: Protocol, WorkDir: plan.Directory, Entry: filepath.Join(plan.Directory, plan.Entry),
		Data: filepath.Join(plan.Directory, plan.Data), Renderer: module(plan.Definition),
		Components: components, View: plan.View, Theme: plan.Theme, Stage: stage, Artifact: plan.Artifact,
	}
}

type ProcessWorker struct {
	// Executable is an absolute, operator-selected path. An empty value
	// selects the embedded engine and looks up Node on PATH.
	Executable string
	Stderr     io.Writer
}

func (worker ProcessWorker) Run(ctx context.Context, job Job) (Receipt, error) {
	if job.Protocol != Protocol {
		return Receipt{}, fmt.Errorf("unsupported worker request protocol %q", job.Protocol)
	}
	executable := worker.Executable
	var args []string
	var directory string
	if executable == "" {
		var err error
		executable, directory, err = embeddedEngine()
		if err != nil {
			return Receipt{}, err
		}
		defer os.RemoveAll(directory)
		args = []string{filepath.Join(directory, "worker.cjs")}
	}
	if !filepath.IsAbs(executable) {
		return Receipt{}, fmt.Errorf("worker executable must be absolute")
	}
	input, err := json.Marshal(job)
	if err != nil {
		return Receipt{}, err
	}
	var stdout receiptBuffer
	cmd := task.Command(ctx, executable, args...)
	cmd.Dir = job.WorkDir
	cmd.Stdin = bytes.NewReader(append(input, '\n'))
	cmd.Stdout = &stdout
	cmd.Stderr = worker.Stderr
	if cmd.Stderr == nil {
		cmd.Stderr = os.Stderr
	}
	var diagnostic diagnosticTail
	if directory != "" {
		cmd.Env = append(cmd.Environ(), "TMPDIR="+directory)
		cmd.Stderr = io.MultiWriter(cmd.Stderr, &diagnostic)
	}
	if err := cmd.Run(); err != nil {
		if ctx.Err() != nil {
			return Receipt{}, ctx.Err()
		}
		if directory != "" {
			var exit *exec.ExitError
			message := strings.TrimSpace(diagnostic.String())
			if _, detail, found := strings.Cut(message, "report engine: "); found {
				message = detail
			}
			if message != "" {
				if errors.As(err, &exit) && exit.ExitCode() == 2 {
					return Receipt{}, fmt.Errorf("%w: %s", ErrInvalid, message)
				}
				return Receipt{}, fmt.Errorf("%s: %w", message, err)
			}
		}
		return Receipt{}, fmt.Errorf("execute %s: %w", executable, err)
	}
	decoder := json.NewDecoder(&stdout.Buffer)
	decoder.DisallowUnknownFields()
	var receipt Receipt
	if err := decoder.Decode(&receipt); err != nil {
		return Receipt{}, fmt.Errorf("invalid worker receipt: %w", err)
	}
	var extra any
	if err := decoder.Decode(&extra); !errors.Is(err, io.EOF) {
		return Receipt{}, fmt.Errorf("worker stdout must contain exactly one receipt")
	}
	return receipt, nil
}

type receiptBuffer struct{ bytes.Buffer }

func (buffer *receiptBuffer) Write(p []byte) (int, error) {
	const limit = 64 << 10
	if buffer.Len()+len(p) > limit {
		return 0, fmt.Errorf("worker receipt exceeds %d bytes", limit)
	}
	return buffer.Buffer.Write(p)
}

// diagnosticTail keeps HTTP errors contextual even when the operator can also
// see the worker's diagnostic stream. Author output cannot grow this buffer.
type diagnosticTail struct{ bytes.Buffer }

func (buffer *diagnosticTail) Write(p []byte) (int, error) {
	const limit = 8192
	length := len(p)
	if length >= limit {
		buffer.Reset()
		p = p[length-limit:]
	} else if excess := buffer.Len() + length - limit; excess > 0 {
		buffer.Next(excess)
	}
	_, _ = buffer.Buffer.Write(p)
	return length, nil
}
